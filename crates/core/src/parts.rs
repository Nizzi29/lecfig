//! The part catalog: every circuit part lecfig knows, with its zap function and pins.
//!
//! Pin offsets come from zap itself (`probe/pins.typ` → `pins.json`), so they match the
//! renderer exactly. Two-terminal parts have no table: their pins are `from` and `to`.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::model::{Element, Pt};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// Drawn between `from` and `to`; pins `in` and `out`.
    TwoTerminal,
    /// Placed at `at` with `rot`/`flip`; pins from the table.
    Point,
}

#[derive(Clone, Debug)]
pub struct Part {
    /// The element `type` in figure JSON.
    pub kind: &'static str,
    pub name: &'static str,
    pub shape: Shape,
    /// Default letter for auto-numbered labels (R → R_1, R_2 …); empty = no label.
    pub prefix: &'static str,
}

const fn two(kind: &'static str, name: &'static str, prefix: &'static str) -> Part {
    Part { kind, name, shape: Shape::TwoTerminal, prefix }
}
const fn point(kind: &'static str, name: &'static str, prefix: &'static str) -> Part {
    Part { kind, name, shape: Shape::Point, prefix }
}

pub const PARTS: &[Part] = &[
    two("resistor", "Resistor", "R"),
    two("impedance", "Impedance", "Z"),
    two("capacitor", "Capacitor", "C"),
    two("pcapacitor", "Polarised capacitor", "C"),
    two("inductor", "Inductor", "L"),
    two("vsource", "Voltage source", "U"),
    two("isource", "Current source", "I"),
    two("acvsource", "AC voltage source", "U"),
    two("dvsource", "Controlled voltage source", "U"),
    two("disource", "Controlled current source", "I"),
    two("battery", "Battery", "U"),
    two("diode", "Diode", "D"),
    two("led", "LED", "D"),
    two("zener", "Zener diode", "D"),
    two("switch", "Switch", "S"),
    two("fuse", "Fuse", "F"),
    two("lamp", "Lamp", "H"),
    two("voltmeter", "Voltmeter", ""),
    two("ammeter", "Ammeter", ""),
    two("potentiometer", "Potentiometer", "R"),
    point("ground", "Ground", ""),
    point("earth", "Earth", ""),
    point("vcc", "Supply +", ""),
    point("vee", "Supply −", ""),
    point("node", "Node dot", ""),
    point("terminal", "Terminal", ""),
    point("block", "Labeled block", ""),
    point("transformer", "Transformer (iron core)", ""),
    point("opamp", "Op-amp", ""),
    point("npn", "npn transistor", "T"),
    point("pnp", "pnp transistor", "T"),
    point("nmos", "n-MOSFET", "T"),
    point("pmos", "p-MOSFET", "T"),
    point("land", "AND", ""),
    point("lor", "OR", ""),
    point("lxor", "XOR", ""),
    point("lnand", "NAND", ""),
    point("lnor", "NOR", ""),
    point("lxnor", "XNOR", ""),
    point("lnot", "NOT", ""),
    point("dflipflop", "D flip-flop", ""),
    point("jkflipflop", "JK flip-flop", ""),
    point("srlatch", "SR latch", ""),
];

pub fn part(kind: &str) -> Option<&'static Part> {
    PARTS.iter().find(|p| p.kind == kind)
}

/// The zap anchors that are connection points, per table part.
fn pin_names(kind: &str) -> &'static [&'static str] {
    match kind {
        "opamp" => &["minus", "plus", "out", "v", "g"],
        "npn" | "pnp" => &["b", "c", "e"],
        "nmos" | "pmos" => &["g", "d", "s"],
        "dflipflop" | "jkflipflop" | "srlatch" => &["pin1", "pin2", "pin3", "pin4", "pin5", "pin6", "pinup", "pindown"],
        _ => &[],
    }
}

fn table() -> &'static HashMap<String, Vec<(String, Pt)>> {
    static TABLE: OnceLock<HashMap<String, Vec<(String, Pt)>>> = OnceLock::new();
    TABLE.get_or_init(|| {
        #[derive(serde::Deserialize)]
        struct Row {
            #[serde(rename = "type")]
            kind: String,
            pins: serde_json::Map<String, serde_json::Value>,
        }
        let rows: Vec<Row> = serde_json::from_str(include_str!("pins.json")).expect("pins.json is valid");
        rows.into_iter()
            .map(|r| {
                let pins = pin_names(&r.kind)
                    .iter()
                    .filter_map(|&k| {
                        let v = r.pins.get(k)?;
                        Some((k.to_string(), Pt(v[0].as_f64()?, v[1].as_f64()?)))
                    })
                    .collect();
                (r.kind, pins)
            })
            .collect()
    })
}

/// Input pin offsets of a zap gate with `n` inputs (zap's IEC gate geometry).
fn gate_inputs(n: u32) -> Vec<(String, Pt)> {
    let (width, min_h, pad, spacing) = (0.95, 0.85, 0.2, 0.4);
    let height = f64::max(min_h, (n.max(1) - 1) as f64 * spacing + 2.0 * pad);
    let inner = height - 2.0 * pad;
    (1..=n)
        .map(|k| {
            let y = if n == 1 { 0.0 } else { height / 2.0 - pad - (k - 1) as f64 * inner / (n - 1) as f64 };
            (format!("in{k}"), Pt(-width / 2.0, y))
        })
        .collect()
}

/// Size and pin counts of a labeled block: (width, height, left pins, right pins).
pub fn block_shape(el: &Element) -> (f64, f64, u32, u32) {
    let size = el.extra.get("size").and_then(|v| v.as_array()).map(|a| {
        (a.first().and_then(|x| x.as_f64()).unwrap_or(3.0), a.get(1).and_then(|x| x.as_f64()).unwrap_or(4.0))
    });
    let (w, h) = size.unwrap_or((3.0, 4.0));
    let n = |k: &str| el.extra.get(k).and_then(|v| v.as_u64()).unwrap_or(2) as u32;
    (w, h, n("inputs"), n("outputs"))
}

/// Offsets of `n` pins along a side of height `h`: spread over the side, centred, and
/// on the grid where possible. Mirrored in typst/lecfig/render-circuit.typ (`side-pins`).
pub fn side_pins(n: u32, h: f64) -> Vec<f64> {
    let mut s = (h / n.max(1) as f64).floor().max(1.0);
    if ((n.max(1) - 1) as f64 * s) % 2.0 == 1.0 && s > 1.0 {
        s -= 1.0;
    }
    (0..n).map(|k| ((n as f64 - 1.0) / 2.0 - k as f64) * s).collect()
}

/// Pin offsets of a point part at rot 0, unflipped, relative to `at`.
pub fn local_pins(el: &Element) -> Vec<(String, Pt)> {
    match el.kind.as_str() {
        // One connection point at the part's position (referenced by the element id).
        "ground" | "earth" | "frame" | "vcc" | "vee" | "node" | "terminal" => vec![(String::new(), Pt(0.0, 0.0))],
        // Primary on the left, secondary on the right; see draw-transformer in Typst.
        "transformer" => vec![
            ("p1".into(), Pt(-1.0, 1.0)),
            ("p2".into(), Pt(-1.0, -1.0)),
            ("s1".into(), Pt(1.0, 1.0)),
            ("s2".into(), Pt(1.0, -1.0)),
        ],
        "block" => {
            let (w, h, nin, nout) = block_shape(el);
            let left = side_pins(nin, h).into_iter().enumerate().map(|(k, y)| (format!("in{}", k + 1), Pt(-w / 2.0, y)));
            let right = side_pins(nout, h).into_iter().enumerate().map(|(k, y)| (format!("out{}", k + 1), Pt(w / 2.0, y)));
            left.chain(right).collect()
        }
        k @ ("land" | "lor" | "lxor" | "lnand" | "lnor" | "lxnor" | "lnot") => {
            let n = if k == "lnot" { 1 } else { el.extra.get("inputs").and_then(|v| v.as_u64()).unwrap_or(2) as u32 };
            let mut pins = gate_inputs(n);
            let out_x = if matches!(k, "lnand" | "lnor" | "lxnor" | "lnot") { 0.655 } else { 0.475 };
            pins.push(("out".into(), Pt(out_x, 0.0)));
            pins
        }
        k => table().get(k).cloned().unwrap_or_default(),
    }
}

/// All pins of an element in figure coordinates, as (reference, position).
/// References are `"<id>.<pin>"`, or just `"<id>"` for single-point parts.
pub fn pins(el: &Element) -> Vec<(String, Pt)> {
    if let (Some(from), Some(to)) = (el.from, el.to) {
        return vec![(format!("{}.in", el.id), from), (format!("{}.out", el.id), to)];
    }
    let at = el.at.unwrap_or_default();
    local_pins(el)
        .into_iter()
        .map(|(name, p)| {
            let p = if el.flip { Pt(p.0, -p.1) } else { p };
            let r = if name.is_empty() { el.id.clone() } else { format!("{}.{name}", el.id) };
            (r, at.add(p.rot90(el.rot)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_pins_match_zap() {
        let el = Element { id: "g".into(), kind: "land".into(), at: Some(Pt(0.0, 0.0)), ..Default::default() };
        let p = pins(&el);
        assert_eq!(p[0].0, "g.in1");
        assert!(p[0].1.dist(Pt(-0.475, 0.225)) < 1e-9);
        assert_eq!(p[2], ("g.out".to_string(), Pt(0.475, 0.0)));
    }

    #[test]
    fn block_pins_spread_on_the_sides() {
        let mut el = Element { id: "b".into(), kind: "block".into(), at: Some(Pt(0.0, 0.0)), ..Default::default() };
        el.extra.insert("size".into(), serde_json::json!([4, 6]));
        el.extra.insert("outputs".into(), serde_json::json!(4));
        let p = pins(&el);
        assert_eq!(p[0], ("b.in1".to_string(), Pt(-2.0, 1.0)));
        assert_eq!(p[1], ("b.in2".to_string(), Pt(-2.0, -1.0)));
        assert_eq!(p[2], ("b.out1".to_string(), Pt(2.0, 1.5)));
        assert_eq!(p.len(), 6);
    }

    #[test]
    fn opamp_pins_rotate() {
        let el = Element { id: "o".into(), kind: "opamp".into(), at: Some(Pt(1.0, 1.0)), rot: 90, ..Default::default() };
        let out = pins(&el).into_iter().find(|(n, _)| n == "o.out").unwrap().1;
        assert!(out.dist(Pt(1.0, 1.9)) < 1e-9);
    }
}
