//! Editing operations on circuits, independent of the UI.
//!
//! Wires connect by position: a wire point that sits on a pin moves with the part.
//! Pin references in wires (`"r1.out"`, from presets) are resolved for display and
//! hit-testing, and kept as they are.

use lecfig_core::model::*;
use lecfig_core::parts::{self, Shape};

pub const EPS: f64 = 1e-6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Item {
    Element(usize),
    Wire(usize),
    Annotation(usize),
}

pub fn same(a: Pt, b: Pt) -> bool {
    a.dist(b) < 1e-4
}

/// All pins in the circuit: (reference, position).
pub fn all_pins(c: &Circuit) -> Vec<(String, Pt)> {
    c.elements.iter().flat_map(parts::pins).collect()
}

/// Position of a wire point: a coordinate, or a pin reference resolved to its pin.
pub fn resolve(c: &Circuit, p: &WirePt) -> Option<Pt> {
    match p {
        WirePt::At(p) => Some(*p),
        WirePt::Pin(r) => all_pins(c).into_iter().find(|(n, _)| n == r).map(|(_, p)| p),
    }
}

pub fn wire_points(c: &Circuit, w: &Wire) -> Vec<Pt> {
    w.points.iter().filter_map(|p| resolve(c, p)).collect()
}

pub fn target_point(c: &Circuit, t: &Target) -> Option<Pt> {
    match t {
        Target::At(p) => Some(*p),
        Target::Anchor(r) => resolve(c, &WirePt::Pin(r.clone())),
        Target::Core(_) => None,
    }
}

/// Bounding box (min, max) of an item in grid units, a little larger than the symbol.
pub fn bbox(c: &Circuit, item: Item) -> Option<(Pt, Pt)> {
    let grow = |pts: &[Pt], m: f64| -> Option<(Pt, Pt)> {
        let first = *pts.first()?;
        let (mut lo, mut hi) = (first, first);
        for p in pts {
            lo = Pt(lo.0.min(p.0), lo.1.min(p.1));
            hi = Pt(hi.0.max(p.0), hi.1.max(p.1));
        }
        Some((Pt(lo.0 - m, lo.1 - m), Pt(hi.0 + m, hi.1 + m)))
    };
    match item {
        Item::Element(i) => {
            let e = c.elements.get(i)?;
            if let (Some(a), Some(b)) = (e.from, e.to) {
                return grow(&[a, b], 0.35);
            }
            let at = e.at?;
            if e.kind == "block" {
                let (w, h, _, _) = parts::block_shape(e);
                let (w, h) = if e.rot.rem_euclid(180) == 90 { (h, w) } else { (w, h) };
                return grow(&[at.add(Pt(-w / 2.0, -h / 2.0)), at.add(Pt(w / 2.0, h / 2.0))], 0.1);
            }
            let mut pts: Vec<Pt> = parts::pins(e).into_iter().map(|(_, p)| p).collect();
            pts.push(at);
            let m = match e.kind.as_str() {
                "node" | "terminal" => 0.2,
                "ground" | "earth" | "vcc" | "vee" => 0.45,
                _ => 0.3,
            };
            grow(&pts, m)
        }
        Item::Wire(i) => grow(&wire_points(c, c.wires.get(i)?), 0.12),
        Item::Annotation(i) => match c.annotations.get(i)? {
            Annotation::Text { at, .. } => grow(&[*at], 0.3),
            Annotation::Voltage { from, to, .. } => grow(&[target_point(c, from)?, target_point(c, to)?], 0.25),
            Annotation::Callout { to, rel, .. } => {
                let p = target_point(c, to)?;
                grow(&[p, p.add(*rel)], 0.2)
            }
        },
    }
}

fn seg_dist(p: Pt, a: Pt, b: Pt) -> f64 {
    let ab = b.sub(a);
    let len2 = ab.0 * ab.0 + ab.1 * ab.1;
    if len2 < EPS {
        return p.dist(a);
    }
    let t = (((p.0 - a.0) * ab.0 + (p.1 - a.1) * ab.1) / len2).clamp(0.0, 1.0);
    p.dist(a.add(ab.scale(t)))
}

/// The item under `p`, preferring parts over wires over annotations.
pub fn hit(c: &Circuit, p: Pt) -> Option<Item> {
    let mut best: Option<(f64, Item)> = None;
    fn consider(best: &mut Option<(f64, Item)>, d: f64, it: Item) {
        if best.is_none_or(|(bd, _)| d < bd) {
            *best = Some((d, it));
        }
    }
    for (i, e) in c.elements.iter().enumerate() {
        let d = if let (Some(a), Some(b)) = (e.from, e.to) {
            seg_dist(p, a, b)
        } else if let Some((lo, hi)) = bbox(c, Item::Element(i)) {
            if p.0 >= lo.0 && p.0 <= hi.0 && p.1 >= lo.1 && p.1 <= hi.1 { 0.1 } else { f64::MAX }
        } else {
            f64::MAX
        };
        if d < 0.4 {
            consider(&mut best, d, Item::Element(i));
        }
    }
    if best.is_some() {
        return best.map(|b| b.1);
    }
    for (i, w) in c.wires.iter().enumerate() {
        let pts = wire_points(c, w);
        for s in pts.windows(2) {
            let d = seg_dist(p, s[0], s[1]);
            if d < 0.25 {
                consider(&mut best, d, Item::Wire(i));
            }
        }
    }
    if best.is_some() {
        return best.map(|b| b.1);
    }
    for i in 0..c.annotations.len() {
        if let Some((lo, hi)) = bbox(c, Item::Annotation(i)) {
            if p.0 >= lo.0 && p.0 <= hi.0 && p.1 >= lo.1 && p.1 <= hi.1 {
                consider(&mut best, 0.0, Item::Annotation(i));
            }
        }
    }
    best.map(|b| b.1)
}

/// The nearest pin within `r` of `p`.
pub fn pin_near(c: &Circuit, p: Pt, r: f64) -> Option<(String, Pt)> {
    all_pins(c).into_iter().filter(|(_, q)| q.dist(p) < r).min_by(|a, b| a.1.dist(p).total_cmp(&b.1.dist(p)))
}

/// Next free numbered label for a prefix: R_1, R_2, … after the highest existing one.
pub fn next_label(c: &Circuit, prefix: &str) -> String {
    if prefix.is_empty() {
        return String::new();
    }
    let mut max = 0;
    let mut scan = |s: &str| {
        if let Some(n) = s.strip_prefix(prefix).and_then(|r| r.strip_prefix('_')).and_then(|r| r.parse::<u32>().ok()) {
            max = max.max(n);
        }
    };
    for e in &c.elements {
        scan(&e.label);
        if let Some(d) = &e.u {
            scan(d.content());
        }
        if let Some(d) = &e.i {
            scan(d.content());
        }
    }
    format!("{prefix}_{}", max + 1)
}

/// A fresh element id for a type: r1, r2, … (or the type name for unprefixed parts).
pub fn next_id(c: &Circuit, kind: &str) -> String {
    let base = match parts::part(kind).map(|p| p.prefix).unwrap_or("") {
        "" => match kind {
            "ground" | "earth" => "gnd".to_string(),
            "block" => "b".to_string(),
            "transformer" => "tr".to_string(),
            "node" => "n".to_string(),
            "terminal" => "t".to_string(),
            k => k.trim_start_matches('l').chars().take(3).collect(),
        },
        p => p.to_lowercase(),
    };
    (1..).map(|n| format!("{base}{n}")).find(|id| c.elements.iter().all(|e| &e.id != id)).unwrap()
}

/// Places a part at `at`. Two-terminal parts run 2 units along `dir`. Returns its index.
pub fn place(c: &mut Circuit, kind: &str, at: Pt, dir: Pt) -> usize {
    let part = parts::part(kind);
    let id = next_id(c, kind);
    let mut e = Element { id, kind: kind.into(), ..Default::default() };
    match part.map(|p| p.shape) {
        Some(Shape::TwoTerminal) => {
            e.from = Some(at);
            e.to = Some(at.add(dir.scale(2.0)));
            let prefix = part.unwrap().prefix;
            // Sources carry their name on the arrow.
            if kind == "vsource" || kind == "acvsource" || kind == "battery" || kind == "dvsource" {
                e.u = Some(Deco::Text(next_label(c, prefix)));
            } else {
                e.label = next_label(c, prefix);
            }
        }
        _ => {
            e.at = Some(at);
            e.label = next_label(c, part.map(|p| p.prefix).unwrap_or(""));
            if kind == "terminal" {
                e.label = String::new();
            }
            if kind == "block" {
                e.extra.insert("size".into(), serde_json::json!([3, 4]));
                e.extra.insert("inputs".into(), serde_json::json!(2));
                e.extra.insert("outputs".into(), serde_json::json!(2));
            }
        }
    }
    c.elements.push(e);
    c.elements.len() - 1
}

/// Moves the given items by `d`; wire points sitting on moved pins follow, and wires
/// stay orthogonal by inserting a jog where a segment would turn diagonal.
pub fn move_items(c: &mut Circuit, items: &[Item], d: Pt) {
    let moved_elements: Vec<usize> = items.iter().filter_map(|i| if let Item::Element(e) = i { Some(*e) } else { None }).collect();
    let moved_pins: Vec<Pt> = moved_elements.iter().flat_map(|&i| parts::pins(&c.elements[i])).map(|(_, p)| p).collect();
    for &i in &moved_elements {
        let e = &mut c.elements[i];
        for p in [&mut e.from, &mut e.to, &mut e.at].into_iter().flatten() {
            *p = p.add(d);
        }
    }
    for (wi, w) in c.wires.iter_mut().enumerate() {
        if items.contains(&Item::Wire(wi)) {
            for p in &mut w.points {
                if let WirePt::At(q) = p {
                    *q = q.add(d);
                }
            }
            continue;
        }
        let before: Vec<Option<Pt>> = w.points.iter().map(|p| if let WirePt::At(q) = p { Some(*q) } else { None }).collect();
        let moves: Vec<bool> = before.iter().map(|q| q.is_some_and(|q| moved_pins.iter().any(|m| same(*m, q)))).collect();
        if !moves.contains(&true) {
            continue;
        }
        let mut pts: Vec<Option<Pt>> = before.clone();
        for (k, m) in moves.iter().enumerate() {
            if *m {
                pts[k] = pts[k].map(|q| q.add(d));
            }
        }
        // Drag the neighbour of a moved end along, so an L stays an L.
        for k in 0..pts.len() {
            if !moves[k] {
                continue;
            }
            for j in [k.wrapping_sub(1), k + 1] {
                if j >= pts.len() || moves[j] || pts.len() <= 2 {
                    continue;
                }
                let (Some(a0), Some(b0)) = (before[k], before[j]) else { continue };
                // Only interior neighbours can follow; wire ends stay on their pins.
                if j == 0 || j == pts.len() - 1 {
                    continue;
                }
                if let Some(q) = &mut pts[j] {
                    if (a0.1 - b0.1).abs() < EPS {
                        q.1 += d.1;
                    } else if (a0.0 - b0.0).abs() < EPS {
                        q.0 += d.0;
                    }
                }
            }
        }
        // Any diagonal left: insert a jog in the middle.
        let mut out: Vec<WirePt> = vec![];
        for k in 0..pts.len() {
            if k > 0 {
                if let (Some(a), Some(b)) = (pts[k - 1], pts[k]) {
                    if (a.0 - b.0).abs() > EPS && (a.1 - b.1).abs() > EPS {
                        let (a0, b0) = (before[k - 1].unwrap_or(a), before[k].unwrap_or(b));
                        if (a0.1 - b0.1).abs() < EPS {
                            let mx = (a.0 + b.0) / 2.0;
                            out.push(WirePt::At(Pt(mx, a.1)));
                            out.push(WirePt::At(Pt(mx, b.1)));
                        } else {
                            let my = (a.1 + b.1) / 2.0;
                            out.push(WirePt::At(Pt(a.0, my)));
                            out.push(WirePt::At(Pt(b.0, my)));
                        }
                    }
                }
            }
            out.push(match pts[k] {
                Some(q) => WirePt::At(q),
                None => w.points[k].clone(),
            });
        }
        w.points = out;
    }
    for (ai, a) in c.annotations.iter_mut().enumerate() {
        if !items.contains(&Item::Annotation(ai)) {
            continue;
        }
        match a {
            Annotation::Text { at, .. } => *at = at.add(d),
            Annotation::Callout { rel, .. } => *rel = rel.add(d),
            Annotation::Voltage { bend, .. } => *bend = Some(bend.unwrap_or(0.5) + d.0 + d.1),
        }
    }
    simplify_wires(c);
}

/// Drops repeated points and merges straight runs.
pub fn simplify_wires(c: &mut Circuit) {
    for w in &mut c.wires {
        let mut out: Vec<WirePt> = vec![];
        for p in w.points.drain(..) {
            if let (Some(WirePt::At(last)), WirePt::At(q)) = (out.last(), &p) {
                if same(*last, *q) {
                    continue;
                }
            }
            if out.len() >= 2 {
                if let (WirePt::At(a), WirePt::At(b), WirePt::At(q)) = (&out[out.len() - 2], &out[out.len() - 1], &p) {
                    let straight = ((a.0 - b.0).abs() < EPS && (b.0 - q.0).abs() < EPS) || ((a.1 - b.1).abs() < EPS && (b.1 - q.1).abs() < EPS);
                    let between = (b.0 - a.0) * (q.0 - b.0) + (b.1 - a.1) * (q.1 - b.1) > 0.0;
                    if straight && between {
                        out.pop();
                    }
                }
            }
            out.push(p);
        }
        w.points = out;
    }
    c.wires.retain(|w| w.points.len() >= 2);
}

/// Rotates items 90° counter-clockwise (`ccw`) or clockwise. A single two-terminal part
/// turns about its start, a single point part in place, several items about `pivot`.
pub fn rotate(c: &mut Circuit, items: &[Item], pivot: Pt, ccw: bool) {
    let deg = if ccw { 90 } else { 270 };
    let turn = |p: Pt, o: Pt| o.add(p.sub(o).rot90(deg));
    if let [Item::Element(i)] = items {
        let e = &mut c.elements[*i];
        if let (Some(a), Some(b)) = (e.from, e.to) {
            e.to = Some(turn(b, a));
        } else {
            e.rot = (e.rot + deg).rem_euclid(360);
        }
        return;
    }
    for it in items {
        match *it {
            Item::Element(i) => {
                let e = &mut c.elements[i];
                for p in [&mut e.from, &mut e.to, &mut e.at].into_iter().flatten() {
                    *p = turn(*p, pivot);
                }
                if e.at.is_some() {
                    e.rot = (e.rot + deg).rem_euclid(360);
                }
            }
            Item::Wire(i) => {
                for p in &mut c.wires[i].points {
                    if let WirePt::At(q) = p {
                        *q = turn(*q, pivot);
                    }
                }
            }
            Item::Annotation(_) => {}
        }
    }
}

/// Flips point parts; reverses two-terminal parts (swaps the ends).
pub fn flip(c: &mut Circuit, items: &[Item]) {
    for it in items {
        if let Item::Element(i) = *it {
            let e = &mut c.elements[i];
            if let (Some(a), Some(b)) = (e.from, e.to) {
                e.from = Some(b);
                e.to = Some(a);
            } else {
                e.flip = !e.flip;
            }
        }
    }
}

/// Lengthens (or shortens, `delta` < 0) a two-terminal part at its end.
pub fn stretch(c: &mut Circuit, i: usize, delta: f64) {
    let e = &mut c.elements[i];
    if let (Some(a), Some(b)) = (e.from, e.to) {
        let len = a.dist(b);
        if len + delta >= 1.0 {
            let dir = b.sub(a).scale(1.0 / len);
            e.to = Some(a.add(dir.scale(len + delta)));
        }
    }
}

/// Changes a labeled block's size (`dw`, `dh`) or pin counts (`din`, `dout`).
pub fn adjust_block(c: &mut Circuit, i: usize, dw: f64, dh: f64, din: i32, dout: i32) {
    let e = &mut c.elements[i];
    if e.kind != "block" {
        return;
    }
    let (w, h, nin, nout) = parts::block_shape(e);
    let (w, h) = ((w + dw).max(1.0), (h + dh).max(1.0));
    e.extra.insert("size".into(), serde_json::json!([w, h]));
    e.extra.insert("inputs".into(), serde_json::json!((nin as i32 + din).clamp(0, 12)));
    e.extra.insert("outputs".into(), serde_json::json!((nout as i32 + dout).clamp(0, 12)));
}

pub fn delete(c: &mut Circuit, items: &[Item]) {
    let mut items = items.to_vec();
    items.sort();
    for it in items.into_iter().rev() {
        match it {
            Item::Element(i) if i < c.elements.len() => {
                c.elements.remove(i);
            }
            Item::Wire(i) if i < c.wires.len() => {
                c.wires.remove(i);
            }
            Item::Annotation(i) if i < c.annotations.len() => {
                c.annotations.remove(i);
            }
            _ => {}
        }
    }
}

/// Number of wire connections at `p`: wire ends count once, wires passing through twice,
/// pins once.
fn degree(c: &Circuit, p: Pt) -> usize {
    let mut n = 0;
    for w in &c.wires {
        let pts = wire_points(c, w);
        for (k, q) in pts.iter().enumerate() {
            if same(*q, p) {
                n += if k == 0 || k == pts.len() - 1 { 1 } else { 2 };
            }
        }
        for s in pts.windows(2) {
            if !same(s[0], p) && !same(s[1], p) && seg_dist(p, s[0], s[1]) < 1e-4 {
                n += 2;
            }
        }
    }
    n + all_pins(c).iter().filter(|(_, q)| same(*q, p)).count()
}

/// Adds junction dots where three or more connections meet at the given points.
pub fn add_junctions(c: &mut Circuit, points: &[Pt]) {
    for &p in points {
        let has_node = c.elements.iter().any(|e| e.kind == "node" && e.at.is_some_and(|a| same(a, p)));
        // A node's own pin is not a connection.
        if !has_node && degree(c, p) >= 3 {
            let id = next_id(c, "node");
            c.elements.push(Element { id, kind: "node".into(), at: Some(p), ..Default::default() });
        }
    }
}

/// Orthogonal route from `a` to `b`: one corner, horizontal first unless `vertical_first`.
pub fn route(a: Pt, b: Pt, vertical_first: bool) -> Vec<Pt> {
    if (a.0 - b.0).abs() < EPS || (a.1 - b.1).abs() < EPS {
        return vec![b];
    }
    let corner = if vertical_first { Pt(a.0, b.1) } else { Pt(b.0, a.1) };
    vec![corner, b]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circuit() -> Circuit {
        let mut c = Circuit::default();
        place(&mut c, "resistor", Pt(0.0, 0.0), Pt(1.0, 0.0));
        place(&mut c, "capacitor", Pt(4.0, 0.0), Pt(0.0, -1.0));
        c.wires.push(Wire { points: vec![WirePt::At(Pt(2.0, 0.0)), WirePt::At(Pt(4.0, 0.0))], ..Default::default() });
        c
    }

    #[test]
    fn placing_numbers_labels() {
        let mut c = circuit();
        place(&mut c, "resistor", Pt(0.0, 3.0), Pt(1.0, 0.0));
        assert_eq!(c.elements[2].label, "R_2");
        assert_eq!(c.elements[2].id, "r2");
        assert_eq!(c.elements[1].to, Some(Pt(4.0, -2.0)));
    }

    #[test]
    fn moving_a_part_keeps_wires_orthogonal() {
        let mut c = circuit();
        move_items(&mut c, &[Item::Element(1)], Pt(0.0, -1.0));
        let pts = wire_points(&c, &c.wires[0]);
        assert_eq!(pts.first(), Some(&Pt(2.0, 0.0)));
        assert_eq!(pts.last(), Some(&Pt(4.0, -1.0)));
        for s in pts.windows(2) {
            assert!((s[0].0 - s[1].0).abs() < EPS || (s[0].1 - s[1].1).abs() < EPS, "diagonal segment {s:?}");
        }
    }

    #[test]
    fn rotate_two_terminal_about_start() {
        let mut c = circuit();
        rotate(&mut c, &[Item::Element(0)], Pt(0.0, 0.0), true);
        assert_eq!(c.elements[0].to, Some(Pt(0.0, 2.0)));
    }

    #[test]
    fn junction_dot_on_t() {
        let mut c = circuit();
        c.wires.push(Wire { points: vec![WirePt::At(Pt(3.0, 0.0)), WirePt::At(Pt(3.0, 2.0))], ..Default::default() });
        add_junctions(&mut c, &[Pt(3.0, 0.0)]);
        assert!(c.elements.iter().any(|e| e.kind == "node" && e.at == Some(Pt(3.0, 0.0))));
    }

    #[test]
    fn hit_prefers_parts() {
        let c = circuit();
        assert_eq!(hit(&c, Pt(1.0, 0.1)), Some(Item::Element(0)));
        assert_eq!(hit(&c, Pt(3.0, 0.05)), Some(Item::Wire(0)));
        assert_eq!(hit(&c, Pt(10.0, 10.0)), None);
    }
}
