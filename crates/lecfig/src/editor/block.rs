//! Block-diagram editing: keys, mouse and overlay for `kind: "block"`.
//!
//! The cursor moves over fletcher cells (column, row; rows grow downwards). Placing
//! nodes in a row chains them: a node placed right after another is connected to it.

use eframe::egui::{self, Color32, Key, Rect, Stroke};
use lecfig_core::model::*;
use serde_json::Map;

use super::edit::Item;
use super::{ACCENT, App, Mode, Target};

fn block(app: &App) -> &Block {
    match &app.fig {
        Figure::Block(b) => b,
        _ => unreachable!("block handler on a non-block figure"),
    }
}

fn block_mut(app: &mut App) -> &mut Block {
    match &mut app.fig {
        Figure::Block(b) => b,
        _ => unreachable!("block handler on a non-block figure"),
    }
}

fn node_at(b: &Block, cell: Pt) -> Option<usize> {
    b.nodes.iter().position(|n| (n.at.0 - cell.0).abs() < 1e-6 && (n.at.1 - cell.1).abs() < 1e-6)
}

fn prefix(kind: &str) -> &'static str {
    match kind {
        "sum" => "s",
        "gain" => "k",
        "integrator" => "i",
        "tf" => "g",
        "function" => "f",
        "branch" => "b",
        _ => "io",
    }
}

pub fn place(app: &mut App, kind: &'static str) {
    let cell = app.cursor;
    if node_at(block(app), cell).is_some() {
        app.message = "this cell already holds a node".into();
        return;
    }
    let b = block(app);
    let base = prefix(kind);
    let id = (1..).map(|n| format!("{base}{n}")).find(|id| b.nodes.iter().all(|n| &n.id != id)).unwrap();
    let label = default_label(b, kind);
    // Chain: connect from the node placed just before, if it sits directly to the left.
    let chain_from = app
        .chain
        .as_ref()
        .and_then(|id| b.nodes.iter().find(|n| &n.id == id))
        .filter(|n| (n.at.1 - cell.1).abs() < 1e-6 && (cell.0 - n.at.0 - 1.0).abs() < 1e-6)
        .map(|n| n.id.clone());
    app.checkpoint();
    let b = block_mut(app);
    b.nodes.push(BNode { id: id.clone(), kind: kind.into(), at: cell, label, extra: Map::new() });
    if let Some(from) = chain_from {
        b.edges.push(BEdge { from, to: id.clone(), ..Default::default() });
    }
    let i = b.nodes.len() - 1;
    app.sel = vec![Item::Element(i)];
    app.chain = Some(id);
    app.last_part = Some(kind);
    app.cursor = Pt(cell.0 + 1.0, cell.1);
    app.message = "placed · the next node to the right connects automatically · c label".into();
}

fn default_label(b: &Block, kind: &str) -> String {
    match kind {
        "gain" => "K".to_string(),
        "tf" => "G(s)".to_string(),
        "function" => "f(dot)".to_string(),
        "io" => if b.nodes.iter().any(|n| n.kind == "io") { "y" } else { "u" }.to_string(),
        _ => String::new(),
    }
}

/// The node that hangs on the cursor while placing (only on a free cell).
pub fn preview(b: &mut Block, kind: &str, cell: Pt) {
    if node_at(b, cell).is_none() {
        let label = default_label(b, kind);
        b.nodes.push(BNode { id: "lecfig-new".into(), kind: kind.into(), at: cell, label, extra: Map::new() });
    }
}

/// The cell whose centre is closest to figure point `p`.
pub fn nearest_cell(app: &App, p: Pt) -> Pt {
    let (mut best, mut cell) = (f64::MAX, app.cursor);
    let cols: Vec<f64> = block(app).nodes.iter().map(|n| n.at.0).collect();
    let rows: Vec<f64> = block(app).nodes.iter().map(|n| n.at.1).collect();
    let span = |v: &[f64]| (v.iter().cloned().fold(0.0, f64::min) - 3.0, v.iter().cloned().fold(0.0, f64::max) + 3.0);
    let ((c0, c1), (r0, r1)) = (span(&cols), span(&rows));
    let mut col = c0;
    while col <= c1 {
        let mut row = r0;
        while row <= r1 {
            let d = app.cell_pos(Pt(col, row)).dist(p);
            if d < best {
                best = d;
                cell = Pt(col, row);
            }
            row += 1.0;
        }
        col += 1.0;
    }
    cell
}

fn cursor_step(app: &mut App, ch: char) -> bool {
    let d = match ch {
        'h' => Pt(-1.0, 0.0),
        'l' => Pt(1.0, 0.0),
        'j' => Pt(0.0, 1.0),
        'k' => Pt(0.0, -1.0),
        'H' => Pt(-5.0, 0.0),
        'L' => Pt(5.0, 0.0),
        'J' => Pt(0.0, 5.0),
        'K' => Pt(0.0, -5.0),
        _ => return false,
    };
    app.cursor = app.cursor.add(d);
    app.chain = None;
    true
}

pub fn key(app: &mut App, ch: Option<char>, key: Option<Key>) {
    match std::mem::replace(&mut app.mode, Mode::Normal) {
        Mode::Normal => normal(app, ch, key),
        Mode::Edge { from, mut bends } => {
            if let Some(c) = ch {
                if cursor_step(app, c) {
                    app.mode = Mode::Edge { from, bends };
                    return;
                }
            }
            match key {
                Some(Key::Enter) => match node_at(block(app), app.cursor) {
                    Some(i) if block(app).nodes[i].id != from => {
                        let to = block(app).nodes[i].id.clone();
                        app.checkpoint();
                        block_mut(app).edges.push(BEdge { from, to, bends, ..Default::default() });
                        let e = block(app).edges.len() - 1;
                        app.sel = vec![Item::Wire(e)];
                        app.message = "line added · + / - sets the sign at its head · c labels it".into();
                    }
                    _ => {
                        bends.push(app.cursor);
                        app.mode = Mode::Edge { from, bends };
                    }
                },
                Some(Key::Backspace) => {
                    bends.pop();
                    app.mode = Mode::Edge { from, bends };
                }
                Some(Key::Escape) => app.message = "line cancelled".into(),
                _ => app.mode = Mode::Edge { from, bends },
            }
        }
        Mode::Move { original, delta } => {
            if let Some(c) = ch {
                let before = app.cursor;
                if cursor_step(app, c) {
                    let d = delta.add(app.cursor.sub(before));
                    let mut fig = original.clone();
                    if let Figure::Block(b) = &mut fig {
                        for it in &app.sel {
                            if let Item::Element(i) = *it {
                                b.nodes[i].at = b.nodes[i].at.add(d);
                            }
                        }
                    }
                    app.fig = fig;
                    app.mode = Mode::Move { original, delta: d };
                    return;
                }
            }
            match key {
                Some(Key::Enter) => {
                    if original != app.fig {
                        let now = std::mem::replace(&mut app.fig, original);
                        app.checkpoint();
                        app.fig = now;
                    }
                }
                Some(Key::Escape) => {
                    app.fig = original;
                    app.cursor = app.cursor.sub(delta);
                }
                _ => app.mode = Mode::Move { original, delta },
            }
        }
        other => app.mode = other,
    }
}

fn normal(app: &mut App, ch: Option<char>, key: Option<Key>) {
    if let Some(c) = ch {
        if cursor_step(app, c) {
            return;
        }
    }
    let at_cursor = node_at(block(app), app.cursor);
    match (ch, key) {
        (Some('.'), _) => {
            if let Some(kind) = app.last_part {
                app.start_place(kind);
            }
        }
        (Some('v'), _) => app.sel = at_cursor.map(Item::Element).into_iter().collect(),
        (_, Some(Key::Tab)) => {
            let n = block(app).edges.len();
            if n > 0 {
                let next = match app.sel.first() {
                    Some(Item::Wire(i)) => (i + 1) % n,
                    _ => 0,
                };
                app.sel = vec![Item::Wire(next)];
            }
        }
        (Some(s @ ('+' | '-' | '0')), _) => {
            let edges: Vec<usize> = app.sel.iter().filter_map(|it| if let Item::Wire(i) = it { Some(*i) } else { None }).collect();
            if edges.is_empty() {
                app.message = "select a line first (Tab or click)".into();
                return;
            }
            app.checkpoint();
            for i in edges {
                block_mut(app).edges[i].sign = if s == '0' { String::new() } else { s.to_string() };
            }
        }
        (Some('c'), _) => match app.sel.first().copied().or(at_cursor.map(Item::Element)) {
            Some(Item::Element(i)) => app.start_insert(Target::BlockNode(i)),
            Some(Item::Wire(i)) => app.start_insert(Target::BlockEdge(i)),
            _ => {}
        },
        (Some('d'), _) | (_, Some(Key::Delete | Key::Backspace)) => {
            let mut items = app.sel.clone();
            if items.is_empty() {
                items.extend(at_cursor.map(Item::Element));
            }
            if items.is_empty() {
                return;
            }
            app.checkpoint();
            let b = block_mut(app);
            let mut nodes: Vec<usize> = items.iter().filter_map(|it| if let Item::Element(i) = it { Some(*i) } else { None }).collect();
            let mut edges: Vec<usize> = items.iter().filter_map(|it| if let Item::Wire(i) = it { Some(*i) } else { None }).collect();
            let ids: Vec<String> = nodes.iter().map(|&i| b.nodes[i].id.clone()).collect();
            for (k, e) in b.edges.iter().enumerate() {
                if ids.contains(&e.from) || ids.contains(&e.to) {
                    edges.push(k);
                }
            }
            edges.sort();
            edges.dedup();
            nodes.sort();
            for i in edges.into_iter().rev() {
                b.edges.remove(i);
            }
            for i in nodes.into_iter().rev() {
                b.nodes.remove(i);
            }
            app.sel.clear();
        }
        (Some('m'), _) => {
            if app.sel.is_empty() {
                app.sel.extend(at_cursor.map(Item::Element));
            }
            if app.sel.iter().any(|it| matches!(it, Item::Element(_))) {
                app.mode = Mode::Move { original: app.fig.clone(), delta: Pt(0.0, 0.0) };
            }
        }
        (Some('w'), _) => match at_cursor {
            Some(i) => {
                app.mode = Mode::Edge { from: block(app).nodes[i].id.clone(), bends: vec![] };
                app.message = "line: move to the target node, Enter (Enter on empty cells bends)".into();
            }
            None => app.message = "put the cursor on the node the line starts from".into(),
        },
        (Some('S'), _) => {
            app.checkpoint();
            let b = block_mut(app);
            b.style.blocks = if b.style.blocks == "laplace" { "time".into() } else { "laplace".into() };
            app.message = format!("integrators drawn as {}", if b.style.blocks == "laplace" { "1/s" } else { "∫" });
        }
        _ => {}
    }
}

/// Figure positions (cm, y down) of node centres and edge polylines.
fn node_pos(app: &App, i: usize) -> Pt {
    let b = block(app);
    let n = &b.nodes[i];
    app.shown_node(&n.id).unwrap_or_else(|| app.cell_pos(n.at))
}

fn edge_points(app: &App, e: &BEdge) -> Vec<Pt> {
    let b = block(app);
    let pos = |id: &str| b.nodes.iter().position(|n| n.id == id).map(|i| node_pos(app, i));
    let mut pts = vec![];
    pts.extend(pos(&e.from));
    pts.extend(e.bends.iter().map(|c| app.cell_pos(*c)));
    pts.extend(pos(&e.to));
    pts
}

fn seg_dist(p: Pt, a: Pt, b: Pt) -> f64 {
    let ab = b.sub(a);
    let len2 = ab.0 * ab.0 + ab.1 * ab.1;
    if len2 < 1e-9 {
        return p.dist(a);
    }
    let t = (((p.0 - a.0) * ab.0 + (p.1 - a.1) * ab.1) / len2).clamp(0.0, 1.0);
    p.dist(a.add(ab.scale(t)))
}

pub fn mouse(app: &mut App, response: &egui::Response, p: Pt) {
    if !response.clicked() {
        if response.dragged_by(egui::PointerButton::Primary) {
            if let Some(pan) = &mut app.pan {
                *pan += response.drag_delta();
            }
        }
        return;
    }
    let cell = nearest_cell(app, p);
    if let Mode::Edge { .. } = app.mode {
        app.cursor = cell;
        key(app, None, Some(Key::Enter));
        return;
    }
    app.cursor = cell;
    let node = (0..block(app).nodes.len()).find(|&i| node_pos(app, i).dist(p) < 0.5);
    let edge = block(app).edges.iter().position(|e| edge_points(app, e).windows(2).any(|s| seg_dist(p, s[0], s[1]) < 0.2));
    app.sel = match (node, edge) {
        (Some(i), _) => vec![Item::Element(i)],
        (None, Some(e)) => vec![Item::Wire(e)],
        _ => vec![],
    };
}

pub fn overlay(app: &App, painter: &egui::Painter, canvas: Rect) {
    let b = block(app);
    let cell = app.to_screen(canvas, app.cell_pos(app.cursor));
    painter.rect_stroke(Rect::from_center_size(cell, egui::vec2(34.0, 26.0)), 4.0, Stroke::new(1.0, Color32::from_rgba_unmultiplied(220, 50, 47, 140)), egui::StrokeKind::Outside);
    for it in &app.sel {
        match *it {
            Item::Element(i) if i < b.nodes.len() => {
                let c = app.to_screen(canvas, node_pos(app, i));
                painter.rect_stroke(Rect::from_center_size(c, egui::vec2(44.0, 34.0)), 4.0, Stroke::new(1.5, ACCENT), egui::StrokeKind::Outside);
            }
            Item::Wire(i) if i < b.edges.len() => {
                let pts: Vec<egui::Pos2> = edge_points(app, &b.edges[i]).iter().map(|p| app.to_screen(canvas, *p)).collect();
                for s in pts.windows(2) {
                    painter.line_segment([s[0], s[1]], Stroke::new(3.0, Color32::from_rgba_unmultiplied(0, 120, 215, 110)));
                }
            }
            _ => {}
        }
    }
    if let Mode::Edge { from, bends } = &app.mode {
        let mut pts = vec![];
        if let Some(i) = b.nodes.iter().position(|n| &n.id == from) {
            pts.push(node_pos(app, i));
        }
        pts.extend(bends.iter().map(|c| app.cell_pos(*c)));
        pts.push(app.cell_pos(app.cursor));
        let screen: Vec<egui::Pos2> = pts.iter().map(|p| app.to_screen(canvas, *p)).collect();
        for s in screen.windows(2) {
            painter.line_segment([s[0], s[1]], Stroke::new(2.0, ACCENT));
        }
    }
}
