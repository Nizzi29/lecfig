//! Circuit editing: keys, mouse and overlay for `kind: "circuit"`.

use eframe::egui::{self, Color32, Key, Rect, Stroke};
use lecfig_core::model::*;
use lecfig_core::parts::{self, Shape};

use super::edit::{self, Item};
use super::{ACCENT, App, Mode, Target};

fn circuit(app: &App) -> &Circuit {
    match &app.fig {
        Figure::Circuit(c) => c,
        _ => unreachable!("circuit handler on a non-circuit"),
    }
}

/// Pins snap the cursor in wire and voltage modes.
fn snapped(app: &App) -> (Pt, Option<String>) {
    match edit::pin_near(circuit(app), app.cursor, 0.36) {
        Some((r, p)) => (p, Some(r)),
        None => (app.cursor, None),
    }
}

/// Items the next command acts on: the selection, else what is under the cursor.
fn targets(app: &App) -> Vec<Item> {
    if !app.sel.is_empty() {
        return app.sel.clone();
    }
    edit::hit(circuit(app), app.cursor).into_iter().collect()
}

/// Adds a part at `at`: two-terminal parts run along `dir`, others are turned by `rot`.
pub fn build(c: &mut Circuit, kind: &str, at: Pt, dir: Pt, rot: i32) -> usize {
    let i = edit::place(c, kind, at, dir);
    if c.elements[i].at.is_some() {
        c.elements[i].rot = rot.rem_euclid(360);
    }
    i
}

/// Puts a part down for good: junction dots, selection, cursor to its end for chaining.
pub fn place(app: &mut App, kind: &'static str, rot: i32) {
    let (at, dir) = (app.cursor, app.dir);
    app.checkpoint();
    let c = app.circuit().unwrap();
    let i = build(c, kind, at, dir, rot);
    let (from, to) = (c.elements[i].from, c.elements[i].to);
    let pts: Vec<Pt> = [from, to, c.elements[i].at].into_iter().flatten().collect();
    edit::add_junctions(c, &pts);
    app.sel = vec![Item::Element(i)];
    app.last_part = Some(kind);
    if let Some(to) = to {
        app.cursor = to;
    }
    if kind == "terminal" || kind == "block" {
        app.start_insert(Target::Label(i));
        if kind == "block" {
            app.message = "block: + / - height · > / < width · ] / [ right pins · } / { left pins".into();
        }
        return;
    }
    let name = parts::part(kind).map(|p| p.name).unwrap_or(kind);
    app.message = format!("{name} placed · r turns it · + / - length · c label · . places another");
}

/// Replaces the figure by `original` moved by `delta` (recomputed from scratch each step).
fn preview_move(app: &mut App, original: &Figure, delta: Pt) {
    let mut fig = original.clone();
    if let Figure::Circuit(c) = &mut fig {
        edit::move_items(c, &app.sel, delta);
    }
    app.fig = fig;
}

fn commit_from(app: &mut App, original: Figure) {
    if original != app.fig {
        let now = std::mem::replace(&mut app.fig, original);
        app.checkpoint();
        app.fig = now;
    }
}

fn cursor_step(app: &mut App, ch: char) -> bool {
    let s = app.step();
    let d = match ch {
        'h' => Pt(-s, 0.0),
        'l' => Pt(s, 0.0),
        'j' => Pt(0.0, -s),
        'k' => Pt(0.0, s),
        'H' => Pt(-5.0 * s, 0.0),
        'L' => Pt(5.0 * s, 0.0),
        'J' => Pt(0.0, -5.0 * s),
        'K' => Pt(0.0, 5.0 * s),
        _ => return false,
    };
    app.cursor = app.cursor.add(d);
    true
}

fn finish_wire(app: &mut App, points: Vec<Pt>) {
    if points.len() < 2 {
        app.message = "wire cancelled".into();
        return;
    }
    app.checkpoint();
    let c = app.circuit().unwrap();
    let ends = [points[0], *points.last().unwrap()];
    c.wires.push(Wire { points: points.into_iter().map(WirePt::At).collect(), ..Default::default() });
    edit::simplify_wires(c);
    edit::add_junctions(c, &ends);
    app.message = "wire added".into();
}

pub fn key(app: &mut App, ch: Option<char>, key: Option<Key>) {
    match std::mem::replace(&mut app.mode, Mode::Normal) {
        Mode::Normal => normal(app, ch, key),
        Mode::Annotate => annotate(app, ch),
        Mode::Wire { mut points } => {
            if let Some(c) = ch {
                if cursor_step(app, c) {
                    app.mode = Mode::Wire { points };
                    return;
                }
                if c == 'o' {
                    app.vertical_first = !app.vertical_first;
                    app.mode = Mode::Wire { points };
                    return;
                }
            }
            match key {
                Some(Key::Enter) => {
                    let (p, pin) = snapped(app);
                    let last = *points.last().unwrap();
                    let on_wire = circuit(app).wires.iter().any(|w| {
                        edit::wire_points(circuit(app), w).windows(2).any(|s| {
                            let (a, b) = (s[0], s[1]);
                            let within = p.0 >= a.0.min(b.0) - 1e-6 && p.0 <= a.0.max(b.0) + 1e-6 && p.1 >= a.1.min(b.1) - 1e-6 && p.1 <= a.1.max(b.1) + 1e-6;
                            within && ((a.0 - b.0).abs() < 1e-6 || (a.1 - b.1).abs() < 1e-6)
                        })
                    });
                    if edit::same(p, last) {
                        finish_wire(app, points);
                        return;
                    }
                    // Arrive at a pin from the side it faces.
                    let vertical_first = match &pin {
                        Some(r) => {
                            let el = circuit(app).elements.iter().find(|e| r.starts_with(&format!("{}.", e.id)) || r == &e.id);
                            match el.and_then(|e| e.at.map(|at| p.sub(at))) {
                                Some(off) if off.0.abs() > off.1.abs() + 1e-6 => true,
                                Some(off) if off.1.abs() > off.0.abs() + 1e-6 => false,
                                _ => app.vertical_first,
                            }
                        }
                        None => app.vertical_first,
                    };
                    points.extend(edit::route(last, p, vertical_first));
                    if (pin.is_some() || on_wire) && points.len() >= 2 {
                        finish_wire(app, points);
                    } else {
                        app.mode = Mode::Wire { points };
                    }
                }
                Some(Key::Escape) => finish_wire(app, points),
                Some(Key::Backspace) => {
                    if points.len() > 1 {
                        points.pop();
                    }
                    app.mode = Mode::Wire { points };
                }
                _ => app.mode = Mode::Wire { points },
            }
        }
        Mode::Move { original, delta } => {
            if let Some(c) = ch {
                let before = app.cursor;
                if cursor_step(app, c) {
                    let d = delta.add(app.cursor.sub(before));
                    preview_move(app, &original, d);
                    app.mode = Mode::Move { original, delta: d };
                    return;
                }
            }
            match key {
                Some(Key::Enter) => {
                    commit_from(app, original);
                    app.message = "moved".into();
                }
                Some(Key::Escape) => {
                    app.fig = original;
                    app.cursor = app.cursor.sub(delta);
                }
                _ => app.mode = Mode::Move { original, delta },
            }
        }
        Mode::Voltage { from } => {
            if let Some(c) = ch {
                if cursor_step(app, c) {
                    app.mode = Mode::Voltage { from };
                    return;
                }
            }
            match key {
                Some(Key::Enter) => {
                    let (to, _) = snapped(app);
                    if edit::same(from, to) {
                        app.message = "pick a second point".into();
                        app.mode = Mode::Voltage { from };
                        return;
                    }
                    let target = |p: Pt, c: &Circuit| match edit::pin_near(c, p, 1e-3) {
                        Some((r, _)) => Target_::Anchor(r),
                        None => Target_::At(p),
                    };
                    app.checkpoint();
                    let c = app.circuit().unwrap();
                    let (f, t) = (target(from, c), target(to, c));
                    c.annotations.push(Annotation::Voltage { from: f, to: t, content: "U".into(), invert: false, bend: None });
                    let i = c.annotations.len() - 1;
                    app.start_insert(Target::Annotation(i));
                }
                Some(Key::Escape) => {}
                _ => app.mode = Mode::Voltage { from },
            }
        }
        other => app.mode = other,
    }
}

use lecfig_core::model::Target as Target_;

fn normal(app: &mut App, ch: Option<char>, key: Option<Key>) {
    if let Some(c) = ch {
        if cursor_step(app, c) {
            return;
        }
    }
    match (ch, key) {
        (Some('.'), _) => match app.last_part {
            Some(kind) => app.start_place(kind),
            None => app.message = "nothing placed yet; space opens the picker".into(),
        },
        (Some('v'), _) => {
            app.sel = edit::hit(circuit(app), app.cursor).into_iter().collect();
        }
        (Some('x'), _) => {
            if let Some(it) = edit::hit(circuit(app), app.cursor) {
                match app.sel.iter().position(|s| *s == it) {
                    Some(k) => {
                        app.sel.remove(k);
                    }
                    None => app.sel.push(it),
                }
            }
        }
        (_, Some(Key::Tab)) => {
            let n = circuit(app).elements.len();
            if n > 0 {
                let next = match app.sel.first() {
                    Some(Item::Element(i)) => (i + 1) % n,
                    _ => 0,
                };
                app.sel = vec![Item::Element(next)];
                let e = &circuit(app).elements[next];
                if let Some(p) = e.at.or(e.from) {
                    app.cursor = p;
                }
            }
        }
        (Some('m'), _) => {
            if app.sel.is_empty() {
                app.sel = targets(app);
            }
            if app.sel.is_empty() {
                app.message = "nothing to move".into();
            } else {
                app.mode = Mode::Move { original: app.fig.clone(), delta: Pt(0.0, 0.0) };
            }
        }
        (Some(r @ ('r' | 'R')), _) => {
            // Rotates the selection; with nothing selected, the next part's direction.
            let items = app.sel.clone();
            if items.is_empty() {
                // Nothing selected: turn the direction of the next part.
                app.dir = app.dir.rot90(if r == 'r' { 90 } else { 270 });
                app.message = format!("next part points {}", direction(app.dir));
                return;
            }
            let pivot = app.cursor;
            let old_to = single_two_terminal(app, &items).map(|e| e.to);
            app.checkpoint();
            edit::rotate(app.circuit().unwrap(), &items, pivot, r == 'r');
            // Keep chaining from the rotated part's new end.
            let ends = single_two_terminal(app, &items).map(|e| (e.from.unwrap(), e.to.unwrap()));
            if let (Some(Some(old)), Some((from, to))) = (old_to, ends) {
                if edit::same(old, app.cursor) {
                    app.cursor = to;
                }
                app.dir = to.sub(from).scale(1.0 / from.dist(to));
            }
            app.sel = items;
        }
        (Some('F'), _) => {
            let items = targets(app);
            app.checkpoint();
            edit::flip(app.circuit().unwrap(), &items);
        }
        (Some(k @ ('>' | '<' | ']' | '[' | '}' | '{')), _) => {
            let items = targets(app);
            app.checkpoint();
            let (dw, din, dout) = match k {
                '>' => (1.0, 0, 0),
                '<' => (-1.0, 0, 0),
                ']' => (0.0, 0, 1),
                '[' => (0.0, 0, -1),
                '}' => (0.0, 1, 0),
                _ => (0.0, -1, 0),
            };
            for it in &items {
                if let Item::Element(i) = *it {
                    edit::adjust_block(app.circuit().unwrap(), i, dw, 0.0, din, dout);
                }
            }
        }
        (Some(s @ ('+' | '-')), _) => {
            let items = targets(app);
            let delta = if s == '+' { app.step() } else { -app.step() };
            let old_to = single_two_terminal(app, &items).and_then(|e| e.to);
            app.checkpoint();
            for it in &items {
                if let Item::Element(i) = *it {
                    edit::stretch(app.circuit().unwrap(), i, delta);
                    edit::adjust_block(app.circuit().unwrap(), i, 0.0, delta, 0, 0);
                }
            }
            let new_to = single_two_terminal(app, &items).and_then(|e| e.to);
            if let (Some(old), Some(to)) = (old_to, new_to) {
                if edit::same(old, app.cursor) {
                    app.cursor = to;
                }
            }
        }
        (Some('d'), _) | (_, Some(Key::Delete | Key::Backspace)) => {
            let items = targets(app);
            if !items.is_empty() {
                app.checkpoint();
                edit::delete(app.circuit().unwrap(), &items);
                app.sel.clear();
                app.message = format!("deleted {}", items.len());
            }
        }
        (Some('y'), _) => {
            let c = circuit(app);
            app.clipboard_set(targets(app).iter().filter_map(|it| if let Item::Element(i) = it { c.elements.get(*i).cloned() } else { None }).collect());
        }
        (Some('p'), _) => paste(app),
        (Some('c'), _) => match targets(app).first() {
            Some(Item::Element(i)) => app.start_insert(Target::Label(*i)),
            Some(Item::Wire(i)) => app.start_insert(Target::WireI(*i)),
            Some(Item::Annotation(i)) => app.start_insert(Target::Annotation(*i)),
            None => app.message = "nothing to label here".into(),
        },
        (Some('w'), _) => {
            let (p, _) = snapped(app);
            app.cursor = p;
            app.mode = Mode::Wire { points: vec![p] };
            app.message = "wire: move, Enter bends, Enter on a pin or twice ends, o corner, Esc ends".into();
        }
        (Some('a'), _) => app.mode = Mode::Annotate,
        (Some('o'), _) => app.vertical_first = !app.vertical_first,
        _ => {}
    }
}

fn direction(d: Pt) -> &'static str {
    if d.0 > 0.5 {
        "right"
    } else if d.0 < -0.5 {
        "left"
    } else if d.1 > 0.5 {
        "up"
    } else {
        "down"
    }
}

fn single_two_terminal<'a>(app: &'a App, items: &[Item]) -> Option<&'a Element> {
    match items {
        [Item::Element(i)] => circuit(app).elements.get(*i).filter(|e| e.from.is_some()),
        _ => None,
    }
}

fn annotate(app: &mut App, ch: Option<char>) {
    let item = targets(app).into_iter().next();
    match ch {
        Some('u') => match item {
            Some(Item::Element(i)) if circuit(app).elements[i].from.is_some() => {
                let e = &circuit(app).elements[i];
                let default = deco_default(&e.label, "U");
                if e.u.is_none() {
                    app.checkpoint();
                    app.circuit().unwrap().elements[i].u = Some(Deco::Text(default));
                }
                app.start_insert(Target::U(i));
            }
            _ => {
                let (p, _) = snapped(app);
                app.mode = Mode::Voltage { from: p };
                app.message = "voltage arrow: move to the second point, Enter".into();
            }
        },
        Some('i') => match item {
            Some(Item::Element(i)) => {
                let e = &circuit(app).elements[i];
                let default = deco_default(&e.label, "I");
                if e.i.is_none() {
                    app.checkpoint();
                    app.circuit().unwrap().elements[i].i = Some(Deco::Text(default));
                }
                app.start_insert(Target::I(i));
            }
            Some(Item::Wire(i)) => {
                if circuit(app).wires[i].i.is_none() {
                    app.checkpoint();
                    app.circuit().unwrap().wires[i].i = Some(Deco::Text("I".into()));
                }
                app.start_insert(Target::WireI(i));
            }
            _ => app.message = "put the cursor on a part or wire for a current arrow".into(),
        },
        Some('n') => place(app, "node", 0),
        Some('t') => place(app, "terminal", 0),
        Some('.') => {
            app.checkpoint();
            let at = app.cursor;
            let c = app.circuit().unwrap();
            c.annotations.push(Annotation::Text { at, content: "x".into(), tone: None });
            let i = c.annotations.len() - 1;
            app.start_insert(Target::Annotation(i));
        }
        Some('c') => {
            let (p, pin) = snapped(app);
            let to = match pin {
                Some(r) => Target_::Anchor(r),
                None => Target_::At(p),
            };
            app.checkpoint();
            let c = app.circuit().unwrap();
            c.annotations.push(Annotation::Callout { to, rel: Pt(1.0, 1.0), content: "x".into(), tone: None });
            let i = c.annotations.len() - 1;
            app.sel = vec![Item::Annotation(i)];
            app.start_insert(Target::Annotation(i));
        }
        _ => {}
    }
}

/// `R_1` → `U_1`; a label without subscript → the bare letter.
fn deco_default(label: &str, letter: &str) -> String {
    match label.split_once('_') {
        Some((_, sub)) if !sub.is_empty() => format!("{letter}_{sub}"),
        _ => letter.to_lowercase(),
    }
}

impl App {
    fn clipboard_set(&mut self, els: Vec<Element>) {
        self.message = format!("copied {}", els.len());
        self.clipboard = els;
    }
}

fn paste(app: &mut App) {
    if app.clipboard.is_empty() {
        app.message = "clipboard empty (y copies)".into();
        return;
    }
    let anchor = app.clipboard[0].at.or(app.clipboard[0].from).unwrap_or_default();
    let d = app.cursor.sub(anchor);
    let clip = app.clipboard.clone();
    app.checkpoint();
    let c = app.circuit().unwrap();
    let mut new = vec![];
    for mut e in clip {
        e.id = edit::next_id(c, &e.kind);
        if let Some(p) = parts::part(&e.kind) {
            if !e.label.is_empty() && !p.prefix.is_empty() {
                e.label = edit::next_label(c, p.prefix);
            }
        }
        for p in [&mut e.from, &mut e.to, &mut e.at].into_iter().flatten() {
            *p = p.add(d);
        }
        c.elements.push(e);
        new.push(Item::Element(c.elements.len() - 1));
    }
    app.sel = new;
}

pub fn mouse(app: &mut App, response: &egui::Response, p: Pt) {
    let shift = response.ctx.input(|i| i.modifiers.shift);
    if let Mode::Wire { .. } = app.mode {
        if response.clicked() {
            app.cursor = app.snap(p);
            if let Some((_, q)) = edit::pin_near(circuit(app), p, 0.36) {
                app.cursor = q;
            }
            key(app, None, Some(Key::Enter));
        } else if response.double_clicked() {
            key(app, None, Some(Key::Escape));
        }
        return;
    }
    if response.clicked() {
        let hit = edit::hit(circuit(app), p);
        app.cursor = app.snap(p);
        match (hit, shift) {
            (Some(it), true) => match app.sel.iter().position(|s| *s == it) {
                Some(k) => {
                    app.sel.remove(k);
                }
                None => app.sel.push(it),
            },
            (Some(it), false) => app.sel = vec![it],
            (None, _) => app.sel.clear(),
        }
    }
    if response.drag_started_by(egui::PointerButton::Primary) {
        let start = response.ctx.input(|i| i.pointer.press_origin()).map(|o| o);
        let _ = start;
        if let Some(it) = edit::hit(circuit(app), p) {
            if !app.sel.contains(&it) {
                app.sel = vec![it];
            }
            app.drag = Some((app.snap(p), app.fig.clone()));
        }
    }
    if response.dragged_by(egui::PointerButton::Primary) {
        match app.drag.clone() {
            Some((start, original)) => {
                let d = app.snap(p).sub(start);
                preview_move(app, &original, d);
            }
            None => {
                // Dragging empty space pans.
                if let Some(pan) = &mut app.pan {
                    *pan += response.drag_delta();
                }
            }
        }
    }
    if response.drag_stopped() {
        if let Some((_, original)) = app.drag.take() {
            commit_from(app, original);
        }
    }
}

pub fn overlay(app: &App, painter: &egui::Painter, canvas: Rect) {
    let c = circuit(app);
    // Pins.
    for (_, p) in edit::all_pins(c) {
        painter.circle_stroke(app.to_screen(canvas, p), 2.5, Stroke::new(1.0, Color32::from_rgb(90, 140, 200)));
    }
    if let Some((_, p)) = edit::pin_near(c, app.cursor, 0.36) {
        painter.circle_filled(app.to_screen(canvas, p), 4.0, Color32::from_rgba_unmultiplied(0, 120, 215, 120));
    }
    // Selection.
    for it in &app.sel {
        if let Some((lo, hi)) = edit::bbox(c, *it) {
            let r = Rect::from_two_pos(app.to_screen(canvas, lo), app.to_screen(canvas, hi));
            painter.rect_filled(r, 3.0, Color32::from_rgba_unmultiplied(0, 120, 215, 28));
            painter.rect_stroke(r, 3.0, Stroke::new(1.2, ACCENT), egui::StrokeKind::Outside);
        }
    }
    // Wire in progress.
    if let Mode::Wire { points } = &app.mode {
        let (end, _) = snapped(app);
        let mut pts: Vec<Pt> = points.clone();
        let last = *pts.last().unwrap();
        pts.extend(edit::route(last, end, app.vertical_first));
        let screen: Vec<egui::Pos2> = pts.iter().map(|p| app.to_screen(canvas, *p)).collect();
        for (k, s) in screen.windows(2).enumerate() {
            let preview = k + 1 >= points.len();
            let col = if preview { Color32::from_rgba_unmultiplied(0, 120, 215, 150) } else { ACCENT };
            painter.line_segment([s[0], s[1]], Stroke::new(2.0, col));
        }
    }
    if let Mode::Voltage { from } = &app.mode {
        let (to, _) = snapped(app);
        painter.line_segment([app.to_screen(canvas, *from), app.to_screen(canvas, to)], Stroke::new(1.5, ACCENT));
    }
    // The direction the next two-terminal part takes.
    if matches!(app.mode, Mode::Normal) && app.sel.is_empty() {
        let a = app.to_screen(canvas, app.cursor);
        let b = app.to_screen(canvas, app.cursor.add(app.dir.scale(0.6)));
        painter.arrow(a, b - a, Stroke::new(1.0, Color32::from_rgba_unmultiplied(220, 50, 47, 120)));
    }
    let _ = Shape::Point;
}
