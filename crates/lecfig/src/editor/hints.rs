//! What the keys do right now: the side panel's content for the current mode, figure
//! kind and selection.

use lecfig_core::model::*;
use lecfig_core::parts::{self, Shape};

use super::edit::{self, Item};
use super::magnetic::{self, MagItem};
use super::{App, Mode, Target};

pub struct Hints {
    /// What the user is doing, e.g. "Placing: resistor".
    pub title: String,
    /// One line of context, e.g. what is selected.
    pub note: Option<String>,
    pub groups: Vec<(&'static str, Vec<(&'static str, String)>)>,
}

fn g(name: &'static str, keys: &[(&'static str, &str)]) -> (&'static str, Vec<(&'static str, String)>) {
    (name, keys.iter().map(|(k, t)| (*k, t.to_string())).collect())
}

pub fn hints(app: &App) -> Hints {
    match &app.mode {
        Mode::Normal => normal(app),
        Mode::Picker(_) | Mode::Presets => Hints {
            title: "Choosing".into(),
            note: Some("the list at the bottom left shows what each key picks".into()),
            groups: vec![g("", &[("key", "pick the entry"), ("other key", "close the list")])],
        },
        Mode::Annotate => Hints {
            title: "Annotate".into(),
            note: Some("acts on the selection, or what the cursor is on".into()),
            groups: vec![g(
                "",
                &[
                    ("u", "voltage arrow: on a part, or from the cursor to a second point"),
                    ("i", "current arrow on a part or wire"),
                    ("n", "node dot at the cursor"),
                    ("t", "terminal at the cursor"),
                    (".", "text at the cursor"),
                    ("c", "callout (text with arrow) to the cursor"),
                    ("other key", "back"),
                ],
            )],
        },
        Mode::Place { kind, .. } => {
            let name = match &app.fig {
                Figure::Block(_) => block_name(kind).to_string(),
                _ => parts::part(kind).map(|p| p.name.to_lowercase()).unwrap_or(kind.to_string()),
            };
            let two = parts::part(kind).is_some_and(|p| p.shape == Shape::TwoTerminal);
            Hints {
                title: format!("Placing: {name}"),
                note: Some("the part hangs on the cursor until you put it down".into()),
                groups: vec![
                    g("move", &[("h j k l", "move by one step"), ("H J K L", "move by five"), ("mouse", "follow the pointer")]),
                    g(
                        "finish",
                        &[
                            ("r / R", if two { "turn the direction (ccw / cw)" } else { "turn it (ccw / cw)" }),
                            ("Enter / click", "put it down"),
                            ("Esc", "drop it"),
                        ],
                    ),
                ],
            }
        }
        Mode::Wire { points } => Hints {
            title: "Drawing a wire".into(),
            note: Some(format!("{} point(s) so far; the light line shows the next stretch", points.len())),
            groups: vec![
                g("route", &[("h j k l", "move the end"), ("o", "corner: horizontal first ↔ vertical first"), ("click", "add a point there")]),
                g(
                    "points",
                    &[
                        ("Enter", "fix a bend; on a pin or wire it ends the wire"),
                        ("Enter twice", "end the wire here"),
                        ("Backspace", "take back the last point"),
                        ("Esc / double-click", "end the wire"),
                    ],
                ),
            ],
        },
        Mode::Move { .. } => Hints {
            title: "Moving".into(),
            note: Some(match &app.fig {
                Figure::Magnetic(_) => "the selected item moves along the core".into(),
                _ => "wires attached to moved parts follow and stay square".into(),
            }),
            groups: vec![g(
                "",
                &match &app.fig {
                    Figure::Magnetic(_) => magnetic_move_keys(app),
                    _ => vec![("h j k l", "move by one step"), ("H J K L", "move by five"), ("Enter", "drop here"), ("Esc", "cancel")],
                },
            )],
        },
        Mode::Voltage { .. } => Hints {
            title: "Voltage arrow".into(),
            note: Some("from the first point to the cursor".into()),
            groups: vec![g("", &[("h j k l", "move to the second point"), ("Enter", "set it, then type the label"), ("Esc", "cancel")])],
        },
        Mode::Edge { bends, .. } => Hints {
            title: "Drawing a line".into(),
            note: Some(format!("{} bend(s) so far", bends.len())),
            groups: vec![g(
                "",
                &[
                    ("h j k l", "move to the next cell"),
                    ("Enter on a node", "connect to it"),
                    ("Enter on empty", "bend the line here"),
                    ("click", "same as Enter at that cell"),
                    ("Backspace", "take back the last bend"),
                    ("Esc", "cancel"),
                ],
            )],
        },
        Mode::Insert { target, .. } => insert(target),
    }
}

fn insert(target: &Target) -> Hints {
    if *target == Target::Expr {
        return Hints {
            title: "Editing the expression".into(),
            note: Some("the figure is regenerated from it; changes made by hand are lost".into()),
            groups: vec![g("", &[("Enter", "regenerate"), ("Esc", "cancel")])],
        };
    }
    Hints {
        title: "Typing a label".into(),
        note: Some("Typst math, shown live in the figure".into()),
        groups: vec![
            g("keys", &[("Enter", "keep it"), ("Esc", "undo the change"), ("empty + Enter", "remove an arrow label")]),
            g(
                "Typst math",
                &[
                    ("R_1", "subscript (R_(a b) for longer ones)"),
                    ("x^2", "superscript"),
                    ("\"AC/DC\"", "plain text in quotes"),
                    ("\\", "line break: \"DC\" \\ \"DC\""),
                    ("alpha, Phi", "Greek letters by name"),
                    ("dot(x), x'", "derivatives"),
                    ("underline(I)", "phasor / complex quantity"),
                    ("1/(j omega C)", "fractions"),
                ],
            ),
        ],
    }
}

fn general(app: &App) -> Vec<(&'static str, String)> {
    let mut v = vec![("u / U", "undo / redo".to_string())];
    if app.fig.source().is_some() {
        v.push(("e", "edit the generator expression".into()));
    }
    v.push(("space P", "presets".into()));
    v.push(("z", "reset the view (wheel zooms, right-drag pans)".into()));
    v.push(("q", "quit (all saved; an empty figure is deleted)".into()));
    v.push(("?", "hide this panel".into()));
    v
}

fn normal(app: &App) -> Hints {
    match &app.fig {
        Figure::Circuit(c) => circuit(app, c),
        Figure::Block(b) => block(app, b),
        Figure::Magnetic(m) => magnetic_hints(app, m),
    }
}

fn circuit(app: &App, c: &Circuit) -> Hints {
    let mut groups = vec![];
    let note;
    let title;
    match app.sel.as_slice() {
        [] => {
            title = "Normal".to_string();
            let mut add: Vec<(&'static str, String)> = vec![("space", "pick a part (it follows the cursor)".into())];
            if let Some(k) = app.last_part {
                let name = parts::part(k).map(|p| p.name.to_lowercase()).unwrap_or(k.to_string());
                add.push((".", format!("again: {name}")));
            }
            add.push(("w", "draw a wire from the cursor".into()));
            add.push(("a", "annotate (voltage, current, node, text …)".into()));
            groups.push(("add", add));
            let mut sel: Vec<(&'static str, String)> = vec![
                ("h j k l", "move the cursor (H J K L: five)".into()),
                ("v / click", "select what is under the cursor".into()),
                ("x / shift-click", "add to the selection".into()),
                ("Tab", "select the next part".into()),
                ("r / R", "turn the direction of the next part".into()),
                ("g", if app.fine { "back to whole grid steps".into() } else { "half grid steps".into() }),
            ];
            if edit::hit(c, app.cursor).is_some() {
                sel.push(("c / m / d", "label / move / delete what the cursor is on".into()));
            }
            groups.push(("select & navigate", sel));
            note = Some(format!("next two-terminal part points {}", direction(app.dir)));
        }
        items => {
            let one = if items.len() == 1 { items.first().copied() } else { None };
            match one {
                Some(Item::Element(i)) => {
                    let e = &c.elements[i];
                    let name = parts::part(&e.kind).map(|p| p.name.to_lowercase()).unwrap_or(e.kind.clone());
                    title = format!("Selected: {name} {}", if e.label.is_empty() { e.id.clone() } else { e.label.clone() });
                    let two = e.from.is_some();
                    let mut edit_keys: Vec<(&'static str, String)> = vec![("c", "change the label".into())];
                    if two {
                        edit_keys.push(("r / R", "turn about its start (ccw / cw)".into()));
                        edit_keys.push(("F", "reverse direction".into()));
                        edit_keys.push(("+ / -", "longer / shorter".into()));
                        edit_keys.push(("a u", if e.u.is_some() { "edit the voltage arrow".into() } else { "add a voltage arrow".into() }));
                        edit_keys.push(("a i", if e.i.is_some() { "edit the current arrow".into() } else { "add a current arrow".into() }));
                    } else {
                        edit_keys.push(("r / R", "turn (ccw / cw)".into()));
                        edit_keys.push(("F", if e.kind == "opamp" { "swap + and −".into() } else { "mirror".into() }));
                    }
                    if e.kind == "block" {
                        let (w, h, nin, nout) = parts::block_shape(e);
                        groups.push(("edit", edit_keys));
                        groups.push((
                            "block",
                            vec![
                                ("+ / -", format!("taller / lower (now {h})")),
                                ("> / <", format!("wider / narrower (now {w})")),
                                ("] / [", format!("right pins ± 1 (now {nout})")),
                                ("} / {", format!("left pins ± 1 (now {nin})")),
                            ],
                        ));
                    } else {
                        if matches!(e.kind.as_str(), "land" | "lor" | "lxor" | "lnand" | "lnor" | "lxnor") {
                            edit_keys.push(("(file)", "\"inputs\": n sets the number of inputs".into()));
                        }
                        groups.push(("edit", edit_keys));
                    }
                    note = Some(format!("pins: {}", parts::pins(e).iter().map(|(r, _)| r.split('.').nth(1).unwrap_or(r)).collect::<Vec<_>>().join(", ")));
                }
                Some(Item::Wire(i)) => {
                    title = "Selected: wire".into();
                    groups.push((
                        "edit",
                        vec![("a i", if c.wires[i].i.is_some() { "edit the current arrow".into() } else { "add a current arrow".into() }), ("c", "edit the current label".into())],
                    ));
                    note = Some(format!("{} points", c.wires[i].points.len()));
                }
                Some(Item::Annotation(i)) => {
                    title = match &c.annotations[i] {
                        Annotation::Text { .. } => "Selected: text",
                        Annotation::Voltage { .. } => "Selected: voltage arrow",
                        Annotation::Callout { .. } => "Selected: callout",
                    }
                    .into();
                    groups.push(("edit", vec![("c", "change the text".into()), ("m", "move (callouts: the text, voltage arrows: the bulge)".into())]));
                    note = None;
                }
                _ => {
                    title = format!("Selected: {} items", items.len());
                    groups.push(("edit", vec![("r / R", "turn all about the cursor".into()), ("F", "mirror / reverse each".into())]));
                    note = None;
                }
            }
            groups.push((
                "selection",
                vec![
                    ("m / drag", "move (wires follow)".into()),
                    ("d / Del", "delete".into()),
                    ("y / p", "copy / paste at the cursor".into()),
                    ("x / shift-click", "add or remove items".into()),
                    ("Esc", "deselect".into()),
                ],
            ));
        }
    }
    groups.push(("general", general(app)));
    Hints { title, note, groups }
}

fn block_name(kind: &str) -> &'static str {
    match kind {
        "sum" => "sum",
        "gain" => "gain",
        "integrator" => "integrator",
        "tf" => "transfer function",
        "function" => "function",
        "branch" => "branch point",
        _ => "input / output",
    }
}

fn block(app: &App, b: &Block) -> Hints {
    let mut groups = vec![];
    let (title, note) = match app.sel.as_slice() {
        [Item::Element(i)] if *i < b.nodes.len() => {
            let n = &b.nodes[*i];
            groups.push((
                "edit",
                vec![
                    ("c", "change the label".into()),
                    ("m", "move (lines follow)".into()),
                    ("w", "draw a line from it (cursor on the node)".into()),
                    ("d", "delete it and its lines".into()),
                    ("Esc", "deselect".into()),
                ],
            ));
            (format!("Selected: {} {}", block_name(&n.kind), n.label), None)
        }
        [Item::Wire(i)] if *i < b.edges.len() => {
            let e = &b.edges[*i];
            groups.push((
                "edit",
                vec![
                    ("+ / -", "sign at the arrow head".into()),
                    ("0", "no sign".into()),
                    ("c", "label the signal".into()),
                    ("d", "delete the line".into()),
                    ("Tab", "next line".into()),
                    ("Esc", "deselect".into()),
                ],
            ));
            (format!("Selected: line {} → {}", e.from, e.to), if e.sign.is_empty() { None } else { Some(format!("sign {}", e.sign)) })
        }
        _ => {
            let mut add: Vec<(&'static str, String)> = vec![("space", "pick a block (it follows the cursor)".into())];
            if let Some(k) = app.last_part {
                add.push((".", format!("again: {}", block_name(k))));
            }
            add.push(("w", "draw a line from the node at the cursor".into()));
            groups.push(("add", add));
            groups.push((
                "select & navigate",
                vec![
                    ("h j k l", "move between cells".into()),
                    ("v / click", "select the node".into()),
                    ("Tab", "select the next line".into()),
                    ("S", if b.style.blocks == "laplace" { "draw integrators as ∫".into() } else { "draw integrators as 1/s".into() }),
                ],
            ));
            ("Normal".into(), Some("a node placed right of the last one connects to it".into()))
        }
    };
    groups.push(("general", general(app)));
    Hints { title, note, groups }
}

fn magnetic_hints(app: &App, m: &Magnetic) -> Hints {
    let mut groups = vec![];
    let sel = magnetic::selected(app);
    let title = match sel {
        None => "Normal".to_string(),
        Some(MagItem::Core) => "Selected: core".into(),
        Some(MagItem::Winding(i)) => format!("Selected: winding {}", m.windings[i].label),
        Some(MagItem::Flux(i)) => format!("Selected: flux {}", if m.fluxes[i].label.is_empty() { &m.fluxes[i].id } else { &m.fluxes[i].label }),
        Some(MagItem::Section(_)) => "Selected: cross-section".into(),
        Some(MagItem::Gap(_)) => "Selected: air gap".into(),
        Some(MagItem::Note(_)) => "Selected: callout".into(),
    };
    let edit_keys: Vec<(&'static str, String)> = match sel {
        None => vec![],
        Some(MagItem::Core) => vec![("m", "resize (h j k l)".into()), ("+ / -", "scale everything".into())],
        Some(MagItem::Winding(i)) => vec![
            ("F", format!("reverse the winding sense (now {})", m.windings[i].sense)),
            ("+ / -", format!("more / fewer turns drawn (now {})", m.windings[i].turns)),
            ("m", "slide along the leg".into()),
            ("c", "change the label".into()),
        ],
        Some(MagItem::Flux(i)) => vec![
            ("F", format!("reverse the direction (now {})", m.fluxes[i].dir)),
            ("+ / -", "move outward / inward in the core".into()),
            ("m", "move the arrow head along the core".into()),
            ("c", "change the label".into()),
        ],
        Some(MagItem::Section(_)) => vec![("m", "slide along the leg".into())],
        Some(MagItem::Gap(_)) => vec![("+ / -", "wider / narrower".into()), ("m", "slide along the leg".into())],
        Some(MagItem::Note(_)) => vec![("m", "move the text".into()), ("c", "change the text".into())],
    };
    if !edit_keys.is_empty() {
        let mut keys = edit_keys;
        if !matches!(sel, Some(MagItem::Core)) {
            keys.push(("d", "delete".into()));
        }
        keys.push(("a", "add a callout pointing at it".into()));
        groups.push(("edit", keys));
    }
    groups.push((
        "select & add",
        vec![
            ("Tab / click", "select the next item".into()),
            ("space", "add a winding, flux, Ψ arrow, cross-section or gap".into()),
        ],
    ));
    groups.push(("general", general(app)));
    Hints { title, note: Some("items sit on the core; editing changes their parameters".into()), groups }
}

fn magnetic_move_keys(app: &App) -> Vec<(&'static str, &'static str)> {
    let mut v = match magnetic::selected(app) {
        Some(MagItem::Core) => vec![("l / h", "wider / narrower"), ("k / j", "taller / lower")],
        Some(MagItem::Note(_)) => vec![("h j k l", "move the text")],
        _ => vec![("h j k l", "slide along the leg")],
    };
    v.push(("Enter", "keep"));
    v.push(("Esc", "cancel"));
    v
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
