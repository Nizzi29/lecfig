//! Magnetic-figure editing: keys, mouse and overlay for `kind: "magnetic"`.
//!
//! Items are attached to the core, so editing changes parameters (sense, turns, lane,
//! position along a leg) rather than free coordinates. The geometry below mirrors
//! `core-legs` / `leg-point` in typst/lecfig/render-magnetic.typ.

use eframe::egui::{self, Color32, Key, Rect, Stroke};
use lecfig_core::model::*;
use serde_json::Map;

use super::edit::Item;
use super::{ACCENT, App, Mode, Target};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum MagItem {
    Core,
    Winding(usize),
    Flux(usize),
    Section(usize),
    Gap(usize),
    Note(usize),
}

struct Leg {
    c: Pt,
    e: Pt,
    n: Pt,
    half: f64,
    len: f64,
}

const LEGS: [&str; 4] = ["left", "right", "top", "bottom"];

fn leg(core: &Core, name: &str) -> Leg {
    let (w, h, a, b) = (core.size.0, core.size.1, core.leg, core.yoke);
    match name {
        "left" => Leg { c: Pt(-w / 2.0 + a / 2.0, 0.0), e: Pt(0.0, 1.0), n: Pt(-1.0, 0.0), half: a / 2.0, len: h - 2.0 * b },
        "right" => Leg { c: Pt(w / 2.0 - a / 2.0, 0.0), e: Pt(0.0, 1.0), n: Pt(1.0, 0.0), half: a / 2.0, len: h - 2.0 * b },
        "top" => Leg { c: Pt(0.0, h / 2.0 - b / 2.0), e: Pt(1.0, 0.0), n: Pt(0.0, 1.0), half: b / 2.0, len: w - 2.0 * a },
        _ => Leg { c: Pt(0.0, -h / 2.0 + b / 2.0), e: Pt(1.0, 0.0), n: Pt(0.0, -1.0), half: b / 2.0, len: w - 2.0 * a },
    }
}

fn leg_point(l: &Leg, pos: f64, lane: f64) -> Pt {
    l.c.add(l.e.scale((pos - 0.5) * l.len)).add(l.n.scale(lane * l.half))
}

fn mag(app: &App) -> &Magnetic {
    match &app.fig {
        Figure::Magnetic(m) => m,
        _ => unreachable!("magnetic handler on another kind"),
    }
}

fn mag_mut(app: &mut App) -> &mut Magnetic {
    match &mut app.fig {
        Figure::Magnetic(m) => m,
        _ => unreachable!("magnetic handler on another kind"),
    }
}

fn items(m: &Magnetic) -> Vec<MagItem> {
    let mut v = vec![MagItem::Core];
    v.extend((0..m.windings.len()).map(MagItem::Winding));
    v.extend((0..m.fluxes.len()).map(MagItem::Flux));
    v.extend((0..m.sections.len()).map(MagItem::Section));
    v.extend((0..m.core.gaps.len()).map(MagItem::Gap));
    v.extend((0..m.annotations.len()).map(MagItem::Note));
    v
}

fn default_span(l: &Leg) -> (f64, f64) {
    if l.e.1 > 0.5 { (0.85, 0.15) } else { (0.15, 0.85) }
}

fn target_pos(m: &Magnetic, t: &lecfig_core::model::Target) -> Pt {
    use lecfig_core::model::Target as T;
    match t {
        T::At(p) => *p,
        T::Core(cp) => {
            let lane = match &cp.flux {
                Some(f) => m.fluxes.iter().find(|x| &x.id == f).map(|x| x.lane).unwrap_or(0.0),
                None => cp.lane.unwrap_or(0.0),
            };
            leg_point(&leg(&m.core, &cp.leg), cp.pos, lane)
        }
        T::Anchor(a) => {
            let (id, anchor) = a.split_once('.').unwrap_or((a.as_str(), ""));
            if let Some(w) = m.windings.iter().find(|w| w.id == id) {
                let l = leg(&m.core, &w.leg);
                let (s0, s1) = w.span.map(|s| (s.0, s.1)).unwrap_or(default_span(&l));
                let pos = if anchor.ends_with('1') { s0 } else { s1 };
                return leg_point(&l, pos, 1.0).add(l.n.scale(0.55));
            }
            if let Some(s) = m.sections.iter().find(|s| s.id == id) {
                return leg_point(&leg(&m.core, &s.leg), s.pos, 0.0).add(Pt(0.0, 0.1));
            }
            Pt(0.0, 0.0)
        }
    }
}

/// Where an item is drawn (for selection and hit-testing).
fn position(m: &Magnetic, it: MagItem) -> Pt {
    match it {
        MagItem::Core => Pt(0.0, m.core.size.1 / 2.0),
        MagItem::Winding(i) => {
            let w = &m.windings[i];
            let l = leg(&m.core, &w.leg);
            let (s0, s1) = w.span.map(|s| (s.0, s.1)).unwrap_or(default_span(&l));
            leg_point(&l, (s0 + s1) / 2.0, 1.0).add(l.n.scale(0.35))
        }
        MagItem::Flux(i) => {
            let f = &m.fluxes[i];
            if f.path == "loop" {
                let a = f.arrows.first().map(|a| (a.leg.as_str(), a.pos)).unwrap_or(("top", 0.5));
                leg_point(&leg(&m.core, a.0), a.1, f.lane)
            } else {
                leg_point(&leg(&m.core, &f.path), 0.5, f.lane)
            }
        }
        MagItem::Section(i) => leg_point(&leg(&m.core, &m.sections[i].leg), m.sections[i].pos, 0.0),
        MagItem::Gap(i) => leg_point(&leg(&m.core, &m.core.gaps[i].leg), m.core.gaps[i].pos, 0.0),
        MagItem::Note(i) => match &m.annotations[i] {
            Annotation::Callout { to, rel, .. } => target_pos(m, to).add(*rel),
            Annotation::Text { at, .. } => *at,
            Annotation::Voltage { from, .. } => target_pos(m, from),
        },
    }
}

pub(super) fn selected(app: &App) -> Option<MagItem> {
    match app.sel.first() {
        Some(Item::Element(k)) => items(mag(app)).get(*k).copied(),
        _ => None,
    }
}

fn select(app: &mut App, it: MagItem) {
    if let Some(k) = items(mag(app)).iter().position(|x| *x == it) {
        app.sel = vec![Item::Element(k)];
        app.message = describe(mag(app), it);
    }
}

fn describe(m: &Magnetic, it: MagItem) -> String {
    match it {
        MagItem::Core => "core · m + hjkl resizes".into(),
        MagItem::Winding(i) => format!("winding {} on the {} leg, {} · F sense · + / - turns · m moves · c label", m.windings[i].label, m.windings[i].leg, m.windings[i].sense),
        MagItem::Flux(i) => format!("flux {} ({}) · F direction · + / - lane · m moves", m.fluxes[i].id, m.fluxes[i].dir),
        MagItem::Section(_) => "cross-section · m moves".into(),
        MagItem::Gap(_) => "air gap · m moves · + / - width".into(),
        MagItem::Note(_) => "callout · m moves the text · c edits it".into(),
    }
}

/// The first leg without a winding (or gap), in the usual order.
fn free_leg(m: &Magnetic) -> &'static str {
    LEGS.iter()
        .find(|l| !m.windings.iter().any(|w| &w.leg == *l) && !m.core.gaps.iter().any(|g| &g.leg == *l))
        .copied()
        .unwrap_or("top")
}

pub fn add(app: &mut App, kind: &str) {
    app.checkpoint();
    let m = mag_mut(app);
    let it = match kind {
        "winding" => {
            let n = m.windings.len() + 1;
            let id = (n..).map(|k| format!("w{k}")).find(|id| m.windings.iter().all(|w| &w.id != id)).unwrap();
            let leg = free_leg(m);
            m.windings.push(Winding {
                id,
                leg: leg.into(),
                sense: "cw".into(),
                turns: 3,
                label: format!("N_{n}"),
                span: None,
                port: Some(Port { i: Some(format!("i_{n}")), i_dir: None, u: Some(format!("u_{n}")), u_dir: None }),
                extra: Map::new(),
            });
            MagItem::Winding(m.windings.len() - 1)
        }
        "flux" => {
            let id = (1..).map(|k| format!("f{k}")).find(|id| m.fluxes.iter().all(|f| &f.id != id)).unwrap();
            m.fluxes.push(Flux {
                id,
                path: "loop".into(),
                lane: 0.0,
                dir: "cw".into(),
                arrows: vec![CorePt { leg: "top".into(), pos: 0.5, lane: None, flux: None }],
                ..Default::default()
            });
            MagItem::Flux(m.fluxes.len() - 1)
        }
        "arrow" => {
            let id = (1..).map(|k| format!("psi{k}")).find(|id| m.fluxes.iter().all(|f| &f.id != id)).unwrap();
            m.fluxes.push(Flux { id, path: "left".into(), lane: 0.1, dir: "cw".into(), span: Some(Pt(0.15, 0.9)), label: "Psi".into(), ..Default::default() });
            MagItem::Flux(m.fluxes.len() - 1)
        }
        "section" => {
            let id = (1..).map(|k| format!("s{k}")).find(|id| m.sections.iter().all(|s| &s.id != id)).unwrap();
            m.sections.push(Section { id, leg: "right".into(), pos: 0.9 });
            MagItem::Section(m.sections.len() - 1)
        }
        _ => {
            let leg = free_leg(m);
            m.core.gaps.push(Gap { leg: leg.into(), pos: 0.5, width: 0.25 });
            MagItem::Gap(m.core.gaps.len() - 1)
        }
    };
    select(app, it);
}

pub fn key(app: &mut App, ch: Option<char>, key: Option<Key>) {
    match std::mem::replace(&mut app.mode, Mode::Normal) {
        Mode::Normal => normal(app, ch, key),
        Mode::Move { original, delta } => {
            if let Some(c) = ch {
                if let Some(d) = step(c) {
                    let total = delta.add(d);
                    let mut fig = original.clone();
                    if let (Figure::Magnetic(m), Some(it)) = (&mut fig, selected(app)) {
                        nudge(m, it, total);
                    }
                    app.fig = fig;
                    app.mode = Mode::Move { original, delta: total };
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
                Some(Key::Escape) => app.fig = original,
                _ => app.mode = Mode::Move { original, delta },
            }
        }
        other => app.mode = other,
    }
}

fn step(c: char) -> Option<Pt> {
    Some(match c {
        'h' => Pt(-1.0, 0.0),
        'l' => Pt(1.0, 0.0),
        'j' => Pt(0.0, -1.0),
        'k' => Pt(0.0, 1.0),
        _ => return None,
    })
}

/// Applies a move of `d` steps (x right, y up) to an item.
fn nudge(m: &mut Magnetic, it: MagItem, d: Pt) {
    let along = |l: &Leg| if l.e.1 > 0.5 { d.1 } else { d.0 };
    match it {
        MagItem::Core => {
            m.core.size = Pt((m.core.size.0 + 0.5 * d.0).max(2.5), (m.core.size.1 + 0.5 * d.1).max(2.0));
        }
        MagItem::Winding(i) => {
            let l = leg(&m.core, &m.windings[i].leg);
            let (s0, s1) = m.windings[i].span.map(|s| (s.0, s.1)).unwrap_or(default_span(&l));
            let dp = 0.05 * along(&l);
            m.windings[i].span = Some(Pt(s0 + dp, s1 + dp));
        }
        MagItem::Flux(i) => {
            let f = &mut m.fluxes[i];
            if f.path == "loop" {
                if let Some(a) = f.arrows.first_mut() {
                    let l = leg(&m.core, &a.leg);
                    a.pos += 0.05 * along(&l);
                }
            } else {
                let l = leg(&m.core, &f.path);
                let (a, b) = f.span.map(|s| (s.0, s.1)).unwrap_or((0.15, 0.85));
                let dp = 0.05 * along(&l);
                f.span = Some(Pt(a + dp, b + dp));
            }
        }
        MagItem::Section(i) => {
            let l = leg(&m.core, &m.sections[i].leg);
            m.sections[i].pos += 0.04 * along(&l);
        }
        MagItem::Gap(i) => {
            let l = leg(&m.core, &m.core.gaps[i].leg);
            m.core.gaps[i].pos += 0.05 * along(&l);
        }
        MagItem::Note(i) => {
            if let Annotation::Callout { rel, .. } | Annotation::Text { at: rel, .. } = &mut m.annotations[i] {
                *rel = rel.add(d.scale(0.25));
            }
        }
    }
}

fn normal(app: &mut App, ch: Option<char>, key: Option<Key>) {
    let all = items(mag(app));
    let cur = selected(app);
    match (ch, key) {
        (_, Some(Key::Tab)) => {
            let k = cur.and_then(|c| all.iter().position(|x| *x == c)).map(|k| k + 1).unwrap_or(0) % all.len();
            select(app, all[k]);
        }
        (Some(c @ ('h' | 'j' | 'k' | 'l')), _) => {
            let d = step(c).unwrap().scale(0.5);
            app.cursor = app.cursor.add(d);
        }
        (Some('F'), _) => {
            let Some(it) = cur else { return };
            app.checkpoint();
            let m = mag_mut(app);
            let flip = |s: &mut String| *s = if s == "cw" { "ccw".into() } else { "cw".into() };
            match it {
                MagItem::Winding(i) => flip(&mut m.windings[i].sense),
                MagItem::Flux(i) => flip(&mut m.fluxes[i].dir),
                _ => {}
            }
            app.message = describe(mag(app), it);
        }
        (Some(s @ ('+' | '-')), _) => {
            let Some(it) = cur else { return };
            let sign = if s == '+' { 1.0 } else { -1.0 };
            app.checkpoint();
            let m = mag_mut(app);
            match it {
                MagItem::Winding(i) => {
                    let t = m.windings[i].turns as i64 + sign as i64;
                    m.windings[i].turns = t.clamp(1, 12) as u32;
                }
                MagItem::Flux(i) => m.fluxes[i].lane = (m.fluxes[i].lane + 0.1 * sign).clamp(-0.9, 0.9),
                MagItem::Gap(i) => m.core.gaps[i].width = (m.core.gaps[i].width + 0.05 * sign).max(0.05),
                MagItem::Core => {
                    m.core.size = m.core.size.scale(1.0 + 0.1 * sign);
                }
                _ => {}
            }
        }
        (Some('m'), _) => {
            if cur.is_some() {
                app.mode = Mode::Move { original: app.fig.clone(), delta: Pt(0.0, 0.0) };
            } else {
                app.message = "select something first (Tab or click)".into();
            }
        }
        (Some('c'), _) => match cur {
            Some(MagItem::Winding(i)) => app.start_insert(Target::Winding(i)),
            Some(MagItem::Flux(i)) => app.start_insert(Target::Flux(i)),
            Some(MagItem::Note(i)) => app.start_insert(Target::Annotation(i)),
            _ => app.message = "select a winding, flux or callout to label".into(),
        },
        (Some('d'), _) | (_, Some(Key::Delete | Key::Backspace)) => {
            let Some(it) = cur else { return };
            app.checkpoint();
            let m = mag_mut(app);
            match it {
                MagItem::Winding(i) => {
                    m.windings.remove(i);
                }
                MagItem::Flux(i) => {
                    m.fluxes.remove(i);
                }
                MagItem::Section(i) => {
                    m.sections.remove(i);
                }
                MagItem::Gap(i) => {
                    m.core.gaps.remove(i);
                }
                MagItem::Note(i) => {
                    m.annotations.remove(i);
                }
                MagItem::Core => app.message = "the core stays".into(),
            }
            app.sel.clear();
        }
        (Some('a'), _) => {
            // A callout pointing at the selected item.
            let Some(it) = cur else {
                app.message = "select what the callout points at".into();
                return;
            };
            let to = position(mag(app), it);
            app.checkpoint();
            let m = mag_mut(app);
            m.annotations.push(Annotation::Callout { to: lecfig_core::model::Target::At(to), rel: Pt(0.8, 0.8), content: "x".into(), tone: None });
            let i = m.annotations.len() - 1;
            select(app, MagItem::Note(i));
            app.start_insert(Target::Annotation(i));
        }
        _ => {}
    }
}

pub fn mouse(app: &mut App, response: &egui::Response, p: Pt) {
    if response.clicked() {
        let m = mag(app);
        let best = items(m)
            .into_iter()
            .map(|it| (position(m, it).dist(p), it))
            .filter(|(d, _)| *d < 0.7)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        app.cursor = p;
        match best {
            Some((_, it)) => select(app, it),
            None => app.sel.clear(),
        }
    } else if response.dragged_by(egui::PointerButton::Primary) {
        if let Some(pan) = &mut app.pan {
            *pan += response.drag_delta();
        }
    }
}

pub fn overlay(app: &App, painter: &egui::Painter, canvas: Rect) {
    let m = mag(app);
    for it in items(m) {
        let s = app.to_screen(canvas, position(m, it));
        painter.circle_stroke(s, 3.0, Stroke::new(1.0, Color32::from_rgb(90, 140, 200)));
    }
    if let Some(it) = selected(app) {
        if it == MagItem::Core {
            let (w, h) = (m.core.size.0, m.core.size.1);
            let r = Rect::from_two_pos(app.to_screen(canvas, Pt(-w / 2.0, -h / 2.0)), app.to_screen(canvas, Pt(w / 2.0, h / 2.0)));
            painter.rect_stroke(r, 2.0, Stroke::new(1.5, ACCENT), egui::StrokeKind::Outside);
        } else {
            let s = app.to_screen(canvas, position(m, it));
            painter.circle_stroke(s, 12.0, Stroke::new(2.0, ACCENT));
        }
    }
}
