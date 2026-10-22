//! The figure editor: the real Typst render of the figure, with a keyboard-first overlay
//! (cursor, pins, selection, wire in progress) drawn on top. Every change is saved at
//! once, so the tinymist preview of the notes follows along.

mod block;
mod circuit;
pub mod edit;
mod magnetic;
mod menu;

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use eframe::egui::{self, Color32, Key, Pos2, Rect, Sense, Stroke, Vec2};
use lecfig_core::model::*;
use lecfig_core::{generators, presets};

use crate::render::{FigWorld, GridTransform, Rendered};
use edit::Item;

/// One centimetre (a circuit grid unit) on screen at zoom 1, in egui points.
const CM: f32 = 38.0;
const PT_PER_CM: f32 = 28.346_457;

pub fn run(path: PathBuf) -> Result<(), String> {
    let fig = crate::course::read_figure(&path)?;
    let name = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    let title = format!("lecfig – {name}");
    let mut viewport = egui::ViewportBuilder::default().with_title(&title).with_inner_size([1100.0, 720.0]).with_app_id("lecfig");
    if std::env::var("LECFIG_SCREENSHOT").is_ok() {
        // Test runs should not take the keyboard from whatever the user is doing.
        viewport = viewport.with_active(false);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(&title, options, Box::new(move |cc| Ok(Box::new(App::new(cc, path, fig)))))
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- render thread

struct Job {
    json: String,
    ppp: f32,
    zoom: f32,
}

struct Done {
    zoom: f32,
    json: String,
    result: Result<Rendered, String>,
}

fn spawn_renderer(root: PathBuf, ctx: egui::Context) -> (Sender<Job>, Receiver<Done>) {
    let (jtx, jrx) = mpsc::channel::<Job>();
    let (dtx, drx) = mpsc::channel::<Done>();
    std::thread::spawn(move || {
        let mut world = FigWorld::new(&root);
        while let Ok(mut job) = jrx.recv() {
            // Only the newest state matters.
            while let Ok(newer) = jrx.try_recv() {
                job = newer;
            }
            let result = world.render(&job.json, job.ppp);
            if dtx.send(Done { zoom: job.zoom, json: job.json, result }).is_err() {
                break;
            }
            ctx.request_repaint();
        }
    });
    (jtx, drx)
}

/// The latest good render: texture plus where its pixels sit in figure coordinates.
struct Shown {
    texture: egui::TextureHandle,
    size: (u32, u32),
    transform: GridTransform,
    /// Block-diagram node centres, in figure coordinates (cm, y down).
    nodes: Vec<(String, Pt)>,
}

// ---------------------------------------------------------------- state

/// What an INSERT edit writes to.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Label(usize),
    U(usize),
    I(usize),
    WireI(usize),
    Annotation(usize),
    BlockNode(usize),
    BlockEdge(usize),
    Winding(usize),
    Flux(usize),
    Expr,
}

pub enum Mode {
    Normal,
    Picker(&'static str),
    Annotate,
    Presets,
    Insert { target: Target, text: String, original: Figure, focus: bool },
    Wire { points: Vec<Pt> },
    Move { original: Figure, delta: Pt },
    /// Second point of a voltage arrow.
    Voltage { from: Pt },
    Edge { from: String, bends: Vec<Pt> },
    /// A new part hangs on the cursor until Enter or a click puts it down.
    Place { kind: &'static str, original: Figure, rot: i32 },
}

pub struct App {
    path: PathBuf,
    pub fig: Figure,
    undo: Vec<Figure>,
    redo: Vec<Figure>,
    pub mode: Mode,
    /// Circuits and magnetic figures: cm, y up. Block diagrams: cells, y down.
    pub cursor: Pt,
    pub sel: Vec<Item>,
    /// Direction of the next two-terminal part.
    pub dir: Pt,
    pub last_part: Option<&'static str>,
    /// Block diagrams: the node placed last, so the next one to its right connects.
    pub chain: Option<String>,
    clipboard: Vec<Element>,
    pub vertical_first: bool,
    pub fine: bool,
    zoom: f32,
    /// Screen offset of figure point (0, 0) from the canvas corner, in points.
    pan: Option<Vec2>,
    jobs: Sender<Job>,
    done: Receiver<Done>,
    requested: (String, f32),
    shown: Option<Shown>,
    /// The figure state the last finished render (good or bad) belongs to.
    rendered_json: String,
    pub error: Option<String>,
    pub message: String,
    saved: String,
    help: bool,
    /// Mouse drag that moves the selection: (start point, original figure).
    drag: Option<(Pt, Figure)>,
    /// Last pointer position seen while placing, so only real mouse moves move the part.
    last_hover: Option<Pos2>,
    /// Debug hook (LECFIG_SCREENSHOT=out.png, LECFIG_KEYS=…): after the first render,
    /// type the keys, save a screenshot of the window and quit.
    script: Option<(String, PathBuf, u8)>,
}

impl App {
    fn new(cc: &eframe::CreationContext, path: PathBuf, fig: Figure) -> Self {
        Self::with_ctx(cc.egui_ctx.clone(), path, fig)
    }

    fn with_ctx(ctx: egui::Context, path: PathBuf, fig: Figure) -> Self {
        let root = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let (jobs, done) = spawn_renderer(root, ctx);
        let saved = fig.to_json();
        let flavor_dir = Pt(1.0, 0.0);
        App {
            path,
            fig,
            undo: vec![],
            redo: vec![],
            mode: Mode::Normal,
            cursor: Pt(0.0, 0.0),
            sel: vec![],
            dir: flavor_dir,
            last_part: None,
            chain: None,
            clipboard: vec![],
            vertical_first: false,
            fine: false,
            zoom: 1.4,
            pan: None,
            jobs,
            done,
            requested: (String::new(), 0.0),
            shown: None,
            rendered_json: String::new(),
            error: None,
            message: "press ? for keys".into(),
            saved,
            help: false,
            drag: None,
            last_hover: None,
            script: std::env::var("LECFIG_SCREENSHOT")
                .ok()
                .map(|p| (std::env::var("LECFIG_KEYS").unwrap_or_default(), PathBuf::from(p), 0)),
        }
    }

    pub fn step(&self) -> f64 {
        if self.fine { 0.5 } else { 1.0 }
    }

    /// Saves the current state for undo. Call before every change.
    pub fn checkpoint(&mut self) {
        self.undo.push(self.fig.clone());
        if self.undo.len() > 500 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.fig, prev));
            self.sel.clear();
            self.message = "undone".into();
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.fig, next));
            self.sel.clear();
            self.message = "redone".into();
        }
    }

    pub fn circuit(&mut self) -> Option<&mut Circuit> {
        match &mut self.fig {
            Figure::Circuit(c) => Some(c),
            _ => None,
        }
    }

    pub fn flavor(&self) -> &'static str {
        match &self.fig {
            Figure::Circuit(c) => match c.flavor.as_str() {
                "equivalent" => "equivalent",
                "schematic" => "schematic",
                "logic" => "logic",
                _ => "network",
            },
            Figure::Block(_) => "block",
            Figure::Magnetic(_) => "magnetic",
        }
    }

    fn save_if_changed(&mut self) {
        // A part hanging on the cursor is not part of the figure yet.
        if matches!(self.mode, Mode::Place { .. }) {
            return;
        }
        let json = self.fig.to_json();
        if json != self.saved {
            match crate::course::write_figure(&self.path, &self.fig) {
                Ok(()) => self.saved = json,
                Err(e) => self.error = Some(format!("save failed: {e}")),
            }
        }
    }

    fn request_render(&mut self, dpr: f32) {
        let json = self.fig.to_json();
        if json != self.requested.0 || (self.zoom - self.requested.1).abs() > 1e-3 {
            let ppp = CM * self.zoom / PT_PER_CM * dpr;
            let _ = self.jobs.send(Job { json: json.clone(), ppp, zoom: self.zoom });
            self.requested = (json, self.zoom);
        }
    }

    fn receive(&mut self, ctx: &egui::Context) {
        while let Ok(done) = self.done.try_recv() {
            self.rendered_json = done.json.clone();
            match done.result {
                Ok(r) => {
                    self.error = None;
                    let image = egui::ColorImage::from_rgba_premultiplied([r.width as usize, r.height as usize], &r.rgba);
                    let texture = ctx.load_texture("figure", image, egui::TextureOptions::LINEAR);
                    // Figure units per pixel: circuits/magnetic from the markers, blocks in cm.
                    let cm_px = CM * done.zoom * ctx.pixels_per_point();
                    let transform = r.transform.unwrap_or(GridTransform { origin: (0.0, 0.0), unit: (cm_px, cm_px) });
                    let nodes = r
                        .nodes
                        .iter()
                        .map(|(id, (x, y))| {
                            let (gx, gy) = transform.to_grid(*x, *y);
                            (id.clone(), Pt(gx, gy))
                        })
                        .collect();
                    self.shown = Some(Shown { texture, size: (r.width, r.height), transform, nodes });
                }
                Err(e) => self.error = Some(e),
            }
        }
    }

    // ------------------------------------------------------------ coordinates

    /// Screen position of a figure point.
    pub fn to_screen(&self, canvas: Rect, p: Pt) -> Pos2 {
        let pan = self.pan.unwrap_or(canvas.center() - canvas.min);
        let u = CM * self.zoom;
        let y_sign = if matches!(self.fig, Figure::Block(_)) { 1.0 } else { -1.0 };
        canvas.min + pan + Vec2::new(p.0 as f32 * u, y_sign * p.1 as f32 * u)
    }

    pub fn to_figure(&self, canvas: Rect, s: Pos2) -> Pt {
        let pan = self.pan.unwrap_or(canvas.center() - canvas.min);
        let u = CM * self.zoom;
        let y_sign = if matches!(self.fig, Figure::Block(_)) { 1.0 } else { -1.0 };
        let d = s - canvas.min - pan;
        Pt((d.x / u) as f64, (y_sign * d.y / u) as f64)
    }

    /// Where the last render put a block-diagram node (figure coordinates).
    pub fn shown_node(&self, id: &str) -> Option<Pt> {
        self.shown.as_ref()?.nodes.iter().find(|(n, _)| n == id).map(|(_, p)| *p)
    }

    pub fn snap(&self, p: Pt) -> Pt {
        let s = self.step();
        Pt((p.0 / s).round() * s, (p.1 / s).round() * s)
    }

    /// Block diagrams: where a cell sits, in figure coordinates (cm, y down), estimated
    /// from the rendered node positions of the same column and row.
    pub fn cell_pos(&self, cell: Pt) -> Pt {
        let Figure::Block(b) = &self.fig else { return cell };
        let nodes: Vec<(Pt, Pt)> = self
            .shown
            .as_ref()
            .map(|s| {
                s.nodes
                    .iter()
                    .filter_map(|(id, pos)| b.nodes.iter().find(|n| &n.id == id).map(|n| (n.at, *pos)))
                    .collect()
            })
            .unwrap_or_default();
        let axis = |k: usize, v: f64| -> f64 {
            let mut known: Vec<(f64, f64)> = vec![];
            for (c, p) in &nodes {
                let (ci, pi) = if k == 0 { (c.0, p.0) } else { (c.1, p.1) };
                if let Some(e) = known.iter_mut().find(|e| (e.0 - ci).abs() < 1e-6) {
                    e.1 = (e.1 + pi) / 2.0;
                } else {
                    known.push((ci, pi));
                }
            }
            known.sort_by(|a, b| a.0.total_cmp(&b.0));
            let spacing = if k == 0 { 2.2 } else { 1.6 };
            match known.len() {
                0 => v * spacing + 1.0,
                1 => known[0].1 + (v - known[0].0) * spacing,
                _ => {
                    if let Some(e) = known.iter().find(|e| (e.0 - v).abs() < 1e-6) {
                        return e.1;
                    }
                    // Interpolate between neighbours, extrapolate with the mean spacing.
                    let lo = known.iter().rev().find(|e| e.0 < v);
                    let hi = known.iter().find(|e| e.0 > v);
                    let mean = (known.last().unwrap().1 - known[0].1) / (known.last().unwrap().0 - known[0].0);
                    match (lo, hi) {
                        (Some(a), Some(b)) => a.1 + (v - a.0) / (b.0 - a.0) * (b.1 - a.1),
                        (Some(a), None) => a.1 + (v - a.0) * mean,
                        (None, Some(b)) => b.1 - (b.0 - v) * mean,
                        _ => v * spacing,
                    }
                }
            }
        };
        Pt(axis(0, cell.0), axis(1, cell.1))
    }

    // ------------------------------------------------------------ actions shared by all kinds

    pub fn start_insert(&mut self, target: Target) {
        let text = self.target_text(&target);
        self.mode = Mode::Insert { target, text, original: self.fig.clone(), focus: true };
    }

    pub fn target_text(&self, t: &Target) -> String {
        match (&self.fig, t) {
            (_, Target::Expr) => self.fig.source().map(|s| s.expr.clone()).unwrap_or_default(),
            (Figure::Circuit(c), Target::Label(i)) => c.elements.get(*i).map(|e| e.label.clone()).unwrap_or_default(),
            (Figure::Circuit(c), Target::U(i)) => {
                c.elements.get(*i).and_then(|e| e.u.as_ref().map(|d| d.content().to_string())).unwrap_or_default()
            }
            (Figure::Circuit(c), Target::I(i)) => {
                c.elements.get(*i).and_then(|e| e.i.as_ref().map(|d| d.content().to_string())).unwrap_or_default()
            }
            (Figure::Circuit(c), Target::WireI(i)) => {
                c.wires.get(*i).and_then(|w| w.i.as_ref().map(|d| d.content().to_string())).unwrap_or_default()
            }
            (fig, Target::Annotation(i)) => annotations(fig).get(*i).map(annotation_text).unwrap_or_default(),
            (Figure::Block(b), Target::BlockNode(i)) => b.nodes.get(*i).map(|n| n.label.clone()).unwrap_or_default(),
            (Figure::Block(b), Target::BlockEdge(i)) => b.edges.get(*i).map(|e| e.label.clone()).unwrap_or_default(),
            (Figure::Magnetic(m), Target::Winding(i)) => m.windings.get(*i).map(|w| w.label.clone()).unwrap_or_default(),
            (Figure::Magnetic(m), Target::Flux(i)) => m.fluxes.get(*i).map(|f| f.label.clone()).unwrap_or_default(),
            _ => String::new(),
        }
    }

    fn apply_text(&mut self, t: &Target, text: &str) {
        let deco = |old: &Option<Deco>| -> Option<Deco> {
            if text.trim().is_empty() {
                None
            } else {
                Some(match old {
                    Some(d) => match d {
                        Deco::Text(_) => Deco::Text(text.into()),
                        Deco::Full { invert, anchor, .. } => Deco::Full { content: text.into(), invert: *invert, anchor: anchor.clone() },
                    },
                    None => Deco::Text(text.into()),
                })
            }
        };
        match (&mut self.fig, t) {
            (_, Target::Expr) => {}
            (Figure::Circuit(c), Target::Label(i)) => {
                if let Some(e) = c.elements.get_mut(*i) {
                    e.label = text.into();
                }
            }
            (Figure::Circuit(c), Target::U(i)) => {
                if let Some(e) = c.elements.get_mut(*i) {
                    e.u = deco(&e.u);
                }
            }
            (Figure::Circuit(c), Target::I(i)) => {
                if let Some(e) = c.elements.get_mut(*i) {
                    e.i = deco(&e.i);
                }
            }
            (Figure::Circuit(c), Target::WireI(i)) => {
                if let Some(w) = c.wires.get_mut(*i) {
                    w.i = deco(&w.i);
                }
            }
            (fig, Target::Annotation(i)) => {
                if let Some(a) = annotations_mut(fig).get_mut(*i) {
                    match a {
                        Annotation::Text { content, .. } | Annotation::Voltage { content, .. } | Annotation::Callout { content, .. } => {
                            *content = text.into()
                        }
                    }
                }
            }
            (Figure::Block(b), Target::BlockNode(i)) => {
                if let Some(n) = b.nodes.get_mut(*i) {
                    n.label = text.into();
                }
            }
            (Figure::Block(b), Target::BlockEdge(i)) => {
                if let Some(e) = b.edges.get_mut(*i) {
                    e.label = text.into();
                }
            }
            (Figure::Magnetic(m), Target::Winding(i)) => {
                if let Some(w) = m.windings.get_mut(*i) {
                    w.label = text.into();
                }
            }
            (Figure::Magnetic(m), Target::Flux(i)) => {
                if let Some(f) = m.fluxes.get_mut(*i) {
                    f.label = text.into();
                }
            }
            _ => {}
        }
    }

    /// Regenerates the figure from its source with a new expression.
    fn regenerate(&mut self, expr: &str) -> Result<(), String> {
        let src = self.fig.source().cloned().ok_or("this figure was not generated")?;
        let mut options = src.options.clone();
        options.remove("steps");
        let fig = generators::generate(&src.generator, expr, &options).map_err(|e| e.0)?;
        self.fig = fig;
        self.sel.clear();
        Ok(())
    }

    /// Picks up a new part: it follows the cursor until it is put down.
    pub fn start_place(&mut self, kind: &'static str) {
        self.mode = Mode::Place { kind, original: self.fig.clone(), rot: 0 };
        self.sel.clear();
        self.update_place();
        self.message = "move with hjkl or the mouse · r / R turns · Enter or click puts it down · Esc drops it".into();
    }

    fn update_place(&mut self) {
        let Mode::Place { kind, original, rot } = &self.mode else { return };
        let mut fig = original.clone();
        match &mut fig {
            Figure::Circuit(c) => {
                circuit::build(c, kind, self.cursor, self.dir, *rot);
            }
            Figure::Block(b) => block::preview(b, kind, self.cursor),
            Figure::Magnetic(_) => {}
        }
        self.fig = fig;
    }

    fn commit_place(&mut self) {
        let Mode::Place { kind, original, rot } = std::mem::replace(&mut self.mode, Mode::Normal) else { return };
        self.fig = original;
        match &self.fig {
            Figure::Circuit(_) => circuit::place(self, kind, rot),
            Figure::Block(_) => block::place(self, kind),
            Figure::Magnetic(_) => {}
        }
    }

    fn cancel_place(&mut self) {
        if let Mode::Place { original, .. } = std::mem::replace(&mut self.mode, Mode::Normal) {
            self.fig = original;
            self.message = "dropped".into();
        }
    }

    /// Moves the cursor for hjkl / HJKL; rows grow downwards in block diagrams.
    fn move_cursor(&mut self, c: char) -> bool {
        let (s, down) = match self.fig {
            Figure::Block(_) => (1.0, 1.0),
            _ => (self.step(), -1.0),
        };
        let d = match c {
            'h' => Pt(-s, 0.0),
            'l' => Pt(s, 0.0),
            'j' => Pt(0.0, down * s),
            'k' => Pt(0.0, -down * s),
            'H' => Pt(-5.0 * s, 0.0),
            'L' => Pt(5.0 * s, 0.0),
            'J' => Pt(0.0, 5.0 * down * s),
            'K' => Pt(0.0, -5.0 * down * s),
            _ => return false,
        };
        self.cursor = self.cursor.add(d);
        true
    }

    pub fn apply_preset(&mut self, name: &str) {
        match crate::user_presets::build(name) {
            Ok(fig) => {
                self.checkpoint();
                self.fig = fig;
                self.sel.clear();
                self.pan = None;
                self.message = format!("preset {name}");
            }
            Err(e) => self.message = e,
        }
    }

    // ------------------------------------------------------------ input

    fn handle_events(&mut self, ctx: &egui::Context) {
        if matches!(self.mode, Mode::Insert { .. }) {
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        for ev in events {
            match ev {
                egui::Event::Text(t) => {
                    for ch in t.chars() {
                        self.key(Some(ch), None, egui::Modifiers::NONE, ctx);
                    }
                }
                egui::Event::Key { key, pressed: true, modifiers, .. } => {
                    let special = matches!(
                        key,
                        Key::Escape | Key::Enter | Key::Tab | Key::Delete | Key::Backspace | Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown
                    );
                    if special || modifiers.command {
                        self.key(None, Some(key), modifiers, ctx);
                    }
                }
                _ => {}
            }
        }
    }

    fn key(&mut self, ch: Option<char>, key: Option<Key>, mods: egui::Modifiers, ctx: &egui::Context) {
        // Shortcuts that work in every mode.
        if mods.command {
            self.cancel_place();
            match key {
                Some(Key::Z) if mods.shift => self.redo(),
                Some(Key::Z) => self.undo(),
                Some(Key::Y) => self.redo(),
                _ => {}
            }
            return;
        }
        // Arrow keys act like hjkl.
        let ch = ch.or(match key {
            Some(Key::ArrowLeft) => Some('h'),
            Some(Key::ArrowDown) => Some('j'),
            Some(Key::ArrowUp) => Some('k'),
            Some(Key::ArrowRight) => Some('l'),
            _ => None,
        });
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Picker(name) => {
                if let Some(c) = ch {
                    let entries = menu::menu(name);
                    if let Some(e) = entries.iter().find(|e| e.key == c) {
                        self.pick(e.action);
                    }
                }
                return;
            }
            Mode::Presets => {
                if let Some(c) = ch {
                    let idx = PICK_KEYS.find(c);
                    if let Some((name, _)) = idx.and_then(|i| preset_list().into_iter().nth(i)) {
                        self.apply_preset(&name);
                    }
                }
                return;
            }
            other => self.mode = other,
        }
        if let Mode::Place { rot, .. } = &mut self.mode {
            match (ch, key) {
                (Some(c @ ('r' | 'R')), _) => {
                    let deg = if c == 'r' { 90 } else { 270 };
                    *rot += deg;
                    self.dir = self.dir.rot90(deg);
                    self.update_place();
                }
                (Some(c), _) if self.move_cursor(c) => self.update_place(),
                (_, Some(Key::Enter)) => self.commit_place(),
                (_, Some(Key::Escape)) => self.cancel_place(),
                _ => {}
            }
            return;
        }
        if key == Some(Key::Escape) && matches!(self.mode, Mode::Normal) {
            self.sel.clear();
            self.help = false;
            return;
        }
        if matches!(self.mode, Mode::Normal) {
            match ch {
                Some('?') => {
                    self.help = !self.help;
                    return;
                }
                Some('u') => return self.undo(),
                Some('U') => return self.redo(),
                Some('q') => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
                Some(' ') => {
                    self.mode = Mode::Picker(self.flavor());
                    return;
                }
                Some('z') => {
                    self.zoom = 1.4;
                    self.pan = None;
                    return;
                }
                Some('g') if !matches!(self.fig, Figure::Magnetic(_)) => {
                    self.fine = !self.fine;
                    self.message = format!("grid {}", self.step());
                    return;
                }
                Some('e') => {
                    if self.fig.source().is_some() {
                        self.start_insert(Target::Expr);
                    } else {
                        self.message = "this figure was not generated; nothing to regenerate".into();
                    }
                    return;
                }
                _ => {}
            }
        }
        match &self.fig {
            Figure::Circuit(_) => circuit::key(self, ch, key),
            Figure::Block(_) => block::key(self, ch, key),
            Figure::Magnetic(_) => magnetic::key(self, ch, key),
        }
    }

    fn pick(&mut self, action: menu::Action) {
        match action {
            menu::Action::Part(kind) => self.start_place(kind),
            menu::Action::Menu(name) => self.mode = Mode::Picker(leak_menu(menu::resolve_menu(name))),
            menu::Action::Presets => self.mode = Mode::Presets,
            menu::Action::Block(kind) => self.start_place(kind),
            menu::Action::Magnetic(kind) => magnetic::add(self, kind),
            menu::Action::ToBlock => {
                if let Figure::Circuit(c) = &self.fig {
                    if c.elements.is_empty() && c.wires.is_empty() {
                        self.checkpoint();
                        self.fig = Figure::Block(Block::default());
                        self.cursor = Pt(0.0, 0.0);
                        self.message = "block diagram: space opens the block picker".into();
                    } else {
                        self.message = "only an empty figure can become a block diagram".into();
                    }
                }
            }
        }
    }
}

const PICK_KEYS: &str = "123456789abcdefghijklmnopqrstuvwxyz";

/// Built-in presets, then yours: (name, title).
fn preset_list() -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = presets::PRESETS.iter().map(|p| (p.name.to_string(), p.title.to_string())).collect();
    v.extend(crate::user_presets::names().into_iter().map(|n| (n.clone(), format!("{n} (yours)"))));
    v
}

fn leak_menu(name: &str) -> &'static str {
    match name {
        "sources" => "sources",
        "electronics" => "electronics",
        "logic-all" => "logic-all",
        "meters" => "meters",
        "misc" => "misc",
        "supply" => "supply",
        _ => "network",
    }
}

pub fn annotations(fig: &Figure) -> &[Annotation] {
    match fig {
        Figure::Circuit(c) => &c.annotations,
        Figure::Block(b) => &b.annotations,
        Figure::Magnetic(m) => &m.annotations,
    }
}

pub fn annotations_mut(fig: &mut Figure) -> &mut Vec<Annotation> {
    match fig {
        Figure::Circuit(c) => &mut c.annotations,
        Figure::Block(b) => &mut b.annotations,
        Figure::Magnetic(m) => &mut m.annotations,
    }
}

fn annotation_text(a: &Annotation) -> String {
    match a {
        Annotation::Text { content, .. } | Annotation::Voltage { content, .. } | Annotation::Callout { content, .. } => content.clone(),
    }
}

// ---------------------------------------------------------------- drawing

pub const ACCENT: Color32 = Color32::from_rgb(0, 120, 215);
pub const CURSOR: Color32 = Color32::from_rgb(220, 50, 47);

/// A figure with nothing drawn in it (magnetic figures always have a core).
fn is_empty(fig: &Figure) -> bool {
    match fig {
        Figure::Circuit(c) => c.elements.is_empty() && c.wires.is_empty() && c.annotations.is_empty(),
        Figure::Block(b) => b.nodes.is_empty() && b.edges.is_empty() && b.annotations.is_empty(),
        Figure::Magnetic(_) => false,
    }
}

impl eframe::App for App {
    /// Closing on an empty canvas removes the figure file, so no empty figures pile up.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.cancel_place();
        if is_empty(&self.fig) {
            let _ = std::fs::remove_file(&self.path);
        } else {
            self.save_if_changed();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.receive(&ctx);
        self.handle_events(&ctx);

        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(Color32::from_gray(236))).show(ui, |ui| {
            let (canvas, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
            self.canvas(ui, canvas, &response);
        });
        self.popups(&ctx);

        self.save_if_changed();
        self.request_render(ctx.pixels_per_point());
        self.run_script(&ctx);
    }
}

impl App {
    fn mode_is_settled(&self) -> bool {
        true
    }

    fn run_script(&mut self, ctx: &egui::Context) {
        let Some((keys, out, stage)) = self.script.clone() else { return };
        match stage {
            // Wait for the first render, then type.
            0 if self.shown.is_some() => {
                for c in keys.chars() {
                    match c {
                        '\n' => self.key(None, Some(Key::Enter), egui::Modifiers::NONE, ctx),
                        '\x1b' => self.key(None, Some(Key::Escape), egui::Modifiers::NONE, ctx),
                        c => self.key(Some(c), None, egui::Modifiers::NONE, ctx),
                    }
                }
                self.script = Some((keys, out, 1));
            }
            // Wait until the render of the current state is on screen.
            1 => {
                if self.rendered_json == self.fig.to_json() && self.mode_is_settled() {
                    self.script = Some((keys, out, 2));
                }
            }
            2..=5 => self.script = Some((keys, out, stage + 1)),
            6 => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                self.script = Some((keys, out, 7));
            }
            _ => {
                let shot = ctx.input(|i| {
                    i.events.iter().find_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None })
                });
                if let Some(img) = shot {
                    let rgba: Vec<u8> = img.pixels.iter().flat_map(|p| p.to_array()).collect();
                    let _ = crate::write_png(&out, img.size[0] as u32, img.size[1] as u32, &rgba);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(30));
    }
}

impl App {
    fn status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let mode = match &self.mode {
                Mode::Normal => "NORMAL",
                Mode::Picker(_) | Mode::Presets => "PICK",
                Mode::Annotate => "ANNOTATE",
                Mode::Insert { .. } => "INSERT",
                Mode::Wire { .. } => "WIRE",
                Mode::Move { .. } => "MOVE",
                Mode::Voltage { .. } => "VOLTAGE",
                Mode::Edge { .. } => "EDGE",
                Mode::Place { .. } => "PLACE",
            };
            ui.label(egui::RichText::new(mode).strong().color(ACCENT));
            ui.separator();
            ui.label(format!("{} · {}", self.fig.kind(), self.flavor()));
            ui.separator();
            ui.label(format!("({}, {})", fmt(self.cursor.0), fmt(self.cursor.1)));
            if !self.sel.is_empty() {
                ui.separator();
                ui.label(format!("{} selected", self.sel.len()));
            }
            ui.separator();
            match &self.error {
                Some(e) => ui.label(egui::RichText::new(format!("Typst: {e}")).color(Color32::from_rgb(200, 30, 30))),
                None => ui.label(&self.message),
            };
        });
    }

    fn canvas(&mut self, ui: &mut egui::Ui, canvas: Rect, response: &egui::Response) {
        let painter = ui.painter_at(canvas);
        // First render: centre the figure.
        if self.pan.is_none() {
            if let Some(s) = &self.shown {
                let (w, h) = s.size;
                let dpr = ui.ctx().pixels_per_point();
                let (ox, oy) = s.transform.origin;
                let img = Vec2::new(w as f32, h as f32) / dpr;
                let origin = Vec2::new(ox, oy) / dpr;
                self.pan = Some((canvas.size() - img) / 2.0 + origin);
            }
        }
        self.mouse(ui, canvas, response);

        // Grid dots.
        let u = CM * self.zoom;
        let lo = self.to_figure(canvas, canvas.left_bottom());
        let hi = self.to_figure(canvas, canvas.right_top());
        let (ylo, yhi) = if lo.1 < hi.1 { (lo.1, hi.1) } else { (hi.1, lo.1) };
        if u > 12.0 {
            for x in (lo.0.floor() as i64)..=(hi.0.ceil() as i64) {
                for y in (ylo.floor() as i64)..=(yhi.ceil() as i64) {
                    painter.circle_filled(self.to_screen(canvas, Pt(x as f64, y as f64)), 1.2, Color32::from_gray(185));
                }
            }
        }

        // The render, placed by its own grid transform.
        if let Some(s) = &self.shown {
            let (w, h) = s.size;
            let tl = s.transform.to_grid(0.0, 0.0);
            let br = s.transform.to_grid(w as f32, h as f32);
            let rect = Rect::from_two_pos(self.to_screen(canvas, Pt(tl.0, tl.1)), self.to_screen(canvas, Pt(br.0, br.1)));
            painter.rect_filled(rect.expand(1.0), 2.0, Color32::from_gray(210));
            painter.image(s.texture.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        } else {
            painter.text(canvas.center(), egui::Align2::CENTER_CENTER, "rendering … (the first render loads fonts and packages)", egui::FontId::proportional(16.0), Color32::GRAY);
        }

        match &self.fig {
            Figure::Circuit(_) => circuit::overlay(self, &painter, canvas),
            Figure::Block(_) => block::overlay(self, &painter, canvas),
            Figure::Magnetic(_) => magnetic::overlay(self, &painter, canvas),
        }

        // Cursor.
        let c = match &self.fig {
            Figure::Block(_) => self.to_screen(canvas, self.cell_pos(self.cursor)),
            _ => self.to_screen(canvas, self.cursor),
        };
        let r = 7.0;
        painter.line_segment([c - Vec2::X * r, c + Vec2::X * r], Stroke::new(1.5, CURSOR));
        painter.line_segment([c - Vec2::Y * r, c + Vec2::Y * r], Stroke::new(1.5, CURSOR));
        painter.circle_stroke(c, 3.5, Stroke::new(1.0, CURSOR));
    }

    fn mouse(&mut self, ui: &egui::Ui, canvas: Rect, response: &egui::Response) {
        // Zoom with the wheel, about the pointer.
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if response.hovered() && scroll.abs() > 0.0 {
            if let Some(m) = response.hover_pos() {
                let before = self.to_figure(canvas, m);
                self.zoom = (self.zoom * (scroll / 400.0).exp()).clamp(0.3, 6.0);
                let after = self.to_screen(canvas, before);
                if let Some(p) = &mut self.pan {
                    *p += m - after;
                }
            }
        }
        // Pan with the middle or right button.
        if response.dragged_by(egui::PointerButton::Middle) || response.dragged_by(egui::PointerButton::Secondary) {
            let d = response.drag_delta();
            if let Some(p) = &mut self.pan {
                *p += d;
            }
            return;
        }
        if matches!(self.mode, Mode::Place { .. }) {
            if let Some(h) = response.hover_pos() {
                if self.last_hover.is_some_and(|l| l != h) {
                    let p = self.to_figure(canvas, h);
                    self.cursor = match self.fig {
                        Figure::Block(_) => block::nearest_cell(self, p),
                        _ => self.snap(p),
                    };
                    self.update_place();
                }
                self.last_hover = Some(h);
            }
            if response.clicked() {
                self.commit_place();
            }
            return;
        }
        self.last_hover = response.hover_pos();
        let Some(pos) = response.interact_pointer_pos() else { return };
        let p = self.to_figure(canvas, pos);
        match &self.fig {
            Figure::Circuit(_) => circuit::mouse(self, response, p),
            Figure::Block(_) => block::mouse(self, response, p),
            Figure::Magnetic(_) => magnetic::mouse(self, response, p),
        }
    }

    fn popups(&mut self, ctx: &egui::Context) {
        // Part picker (which-key style).
        let picker: Option<(String, Vec<(String, String)>)> = match &self.mode {
            Mode::Picker(name) => Some((
                format!("{name}"),
                menu::menu(name).iter().map(|e| (e.key.to_string(), e.label.to_string())).collect(),
            )),
            Mode::Presets => Some((
                "presets".into(),
                preset_list()
                    .into_iter()
                    .zip(PICK_KEYS.chars())
                    .map(|((_, title), k)| (k.to_string(), title))
                    .collect(),
            )),
            Mode::Annotate => Some((
                "annotate".into(),
                [
                    ("u", "voltage arrow (part, or between two points)"),
                    ("i", "current arrow (part or wire)"),
                    ("n", "node dot at the cursor"),
                    ("t", "terminal at the cursor"),
                    (".", "text at the cursor"),
                    ("c", "callout to what the cursor is on"),
                ]
                .iter()
                .map(|(k, l)| (k.to_string(), l.to_string()))
                .collect(),
            )),
            _ => None,
        };
        if let Some((title, entries)) = picker {
            egui::Window::new(title)
                .anchor(egui::Align2::LEFT_BOTTOM, Vec2::new(12.0, -40.0))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    egui::Grid::new("picker").num_columns(2).spacing([14.0, 3.0]).show(ui, |ui| {
                        for (k, l) in entries {
                            ui.label(egui::RichText::new(k).monospace().strong().color(ACCENT));
                            ui.label(l);
                            ui.end_row();
                        }
                    });
                });
        }

        // INSERT: one line of Typst math, applied live.
        let mut finish: Option<bool> = None;
        if let Mode::Insert { target, text, focus, .. } = &mut self.mode {
            let title = match target {
                Target::Expr => "expression (Enter regenerates)",
                Target::U(_) => "voltage (Typst math)",
                Target::I(_) | Target::WireI(_) => "current (Typst math)",
                _ => "label (Typst math)",
            };
            egui::Window::new(title)
                .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 16.0))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    let r = ui.add(egui::TextEdit::singleline(text).desired_width(360.0).font(egui::TextStyle::Monospace));
                    if *focus {
                        r.request_focus();
                        *focus = false;
                    }
                    if ui.input(|i| i.key_pressed(Key::Enter)) {
                        finish = Some(true);
                    } else if ui.input(|i| i.key_pressed(Key::Escape)) {
                        finish = Some(false);
                    }
                });
        }
        if let Mode::Insert { target, text, .. } = &self.mode {
            let (target, text) = (target.clone(), text.clone());
            if target != Target::Expr {
                self.apply_text(&target, &text);
            }
            match finish {
                Some(true) => {
                    let Mode::Insert { original, .. } = std::mem::replace(&mut self.mode, Mode::Normal) else { unreachable!() };
                    if target == Target::Expr {
                        self.fig = original.clone();
                        self.checkpoint();
                        if let Err(e) = self.regenerate(&text) {
                            self.undo.pop();
                            self.message = e;
                        } else {
                            self.message = "regenerated".into();
                        }
                    } else if original != self.fig {
                        let now = std::mem::replace(&mut self.fig, original);
                        self.checkpoint();
                        self.fig = now;
                    }
                }
                Some(false) => {
                    let Mode::Insert { original, .. } = std::mem::replace(&mut self.mode, Mode::Normal) else { unreachable!() };
                    self.fig = original;
                }
                None => {}
            }
        }

        if self.help {
            egui::Window::new("keys").anchor(egui::Align2::RIGHT_TOP, Vec2::new(-12.0, 12.0)).collapsible(false).show(ctx, |ui| {
                ui.label(egui::RichText::new(HELP).monospace().size(12.0));
            });
        }
    }
}

fn fmt(x: f64) -> String {
    let r = (x * 100.0).round() / 100.0;
    if r.fract() == 0.0 { format!("{}", r as i64) } else { format!("{r}") }
}

const HELP: &str = "\
h j k l / arrows  move cursor (H J K L: 5 steps)
space             part picker (flavor first, then submenus);
                  the part follows the cursor (hjkl or mouse),
                  r / R turns it, Enter or click puts it down,
                  Esc drops it
.                 pick up the last part again
v / click         select · x add to selection · Esc clear
m                 move selection (hjkl, Enter drops, Esc cancels)
drag              move selection with the mouse
r / R             rotate ccw / cw     F flip / reverse
+ / -             longer / shorter part, taller / lower block
> < ] [ } {       block: wider / narrower, right pins ±1,
                  left pins ±1
d / Del           delete              y / p  copy / paste
c                 change label (Typst math, live)
w                 wire: hjkl + Enter bends, Enter on a pin
                  or the same point ends, o toggles corner
a                 annotate: u voltage, i current, n node,
                  t terminal, . text, c callout
e                 edit the generator expression, regenerate
space P           presets
g                 half-unit grid on/off
u / U             undo / redo (also Ctrl+Z / Ctrl+Y)
z                 reset view · wheel zoom · right-drag pan
q                 quit (every change is already saved;
                  an empty figure's file is deleted)

block diagrams: space adds a node, w draws a line from
the node at the cursor (Enter on empty cells bends),
+ / - / 0 set the sign at a selected line's arrow head,
Tab selects the next line, S switches ∫ ↔ 1/s

magnetic: Tab cycles items, F flips sense/direction,
+ / - turns or lane, m + hjkl moves, c edits the label";

#[cfg(test)]
mod tests {
    use super::*;

    /// Feeds keys as the window would: characters as text, the rest as keys.
    fn keys(app: &mut App, ctx: &egui::Context, seq: &str) {
        let mut chars = seq.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '<' {
                let name: String = chars.by_ref().take_while(|c| *c != '>').collect();
                let key = match name.as_str() {
                    "Enter" => Key::Enter,
                    "Esc" => Key::Escape,
                    "Tab" => Key::Tab,
                    other => panic!("unknown key <{other}>"),
                };
                app.key(None, Some(key), egui::Modifiers::NONE, ctx);
            } else {
                app.key(Some(c), None, egui::Modifiers::NONE, ctx);
            }
        }
    }

    fn app(fig: Figure) -> (App, egui::Context) {
        let ctx = egui::Context::default();
        let path = std::env::temp_dir().join("lecfig-editor-test.json");
        (App::with_ctx(ctx.clone(), path, fig), ctx)
    }

    #[test]
    fn draw_an_rc_loop_by_keyboard() {
        let (mut a, ctx) = app(Figure::Circuit(Circuit::default()));
        // Source up, resistor right, capacitor down, wire back, ground.
        keys(&mut a, &ctx, " ur<Enter> rR<Enter> cR<Enter>");
        keys(&mut a, &ctx, "<Esc>whh<Enter>");
        keys(&mut a, &ctx, " g<Enter>");
        let Figure::Circuit(c) = &a.fig else { panic!() };
        let get = |k: &str| c.elements.iter().find(|e| e.kind == k).unwrap();
        assert_eq!((get("vsource").from, get("vsource").to), (Some(Pt(0.0, 0.0)), Some(Pt(0.0, 2.0))));
        assert_eq!((get("resistor").from, get("resistor").to), (Some(Pt(0.0, 2.0)), Some(Pt(2.0, 2.0))));
        assert_eq!((get("capacitor").from, get("capacitor").to), (Some(Pt(2.0, 2.0)), Some(Pt(2.0, 0.0))));
        assert_eq!(get("resistor").label, "R_1");
        assert_eq!(get("vsource").u, Some(Deco::Text("U_1".into())));
        assert_eq!(get("ground").at, Some(Pt(0.0, 0.0)));
        assert_eq!(c.wires.len(), 1);
        assert_eq!(edit::wire_points(c, &c.wires[0]), vec![Pt(2.0, 0.0), Pt(0.0, 0.0)]);
        std::fs::write(std::env::temp_dir().join("lecfig-rc.json"), a.fig.to_json()).unwrap();
    }

    #[test]
    fn move_undo_redo() {
        let (mut a, ctx) = app(Figure::Circuit(Circuit::default()));
        keys(&mut a, &ctx, " r<Enter>");
        keys(&mut a, &ctx, "mkk<Enter>");
        let Figure::Circuit(c) = &a.fig else { panic!() };
        assert_eq!(c.elements[0].from, Some(Pt(0.0, 2.0)));
        keys(&mut a, &ctx, "u");
        let Figure::Circuit(c) = &a.fig else { panic!() };
        assert_eq!(c.elements[0].from, Some(Pt(0.0, 0.0)));
        keys(&mut a, &ctx, "uU");
        let Figure::Circuit(c) = &a.fig else { panic!() };
        assert_eq!(c.elements.len(), 1);
    }

    #[test]
    fn block_chain_connects() {
        let (mut a, ctx) = app(Figure::Block(Block::default()));
        keys(&mut a, &ctx, " e<Enter> s<Enter> k<Enter> i<Enter> e<Enter>");
        let Figure::Block(b) = &a.fig else { panic!() };
        assert_eq!(b.nodes.len(), 5);
        assert_eq!(b.edges.len(), 4);
        // A feedback line from the integrator back to the sum, below the row.
        keys(&mut a, &ctx, "hhwj<Enter>hh<Enter>k<Enter><Tab>");
        let Figure::Block(b) = &a.fig else { panic!() };
        assert_eq!(b.edges.len(), 5);
        assert_eq!(b.edges[4].bends, vec![Pt(3.0, 1.0), Pt(1.0, 1.0)]);
    }

    #[test]
    fn part_follows_the_cursor_until_put_down() {
        let (mut a, ctx) = app(Figure::Circuit(Circuit::default()));
        keys(&mut a, &ctx, " r");
        assert!(matches!(a.mode, Mode::Place { .. }));
        keys(&mut a, &ctx, "llk");
        let Figure::Circuit(c) = &a.fig else { panic!() };
        assert_eq!(c.elements[0].from, Some(Pt(2.0, 1.0)), "preview follows the cursor");
        assert!(a.undo.is_empty(), "nothing committed while placing");
        keys(&mut a, &ctx, "<Esc>");
        let Figure::Circuit(c) = &a.fig else { panic!() };
        assert!(c.elements.is_empty(), "Esc drops the part");
        keys(&mut a, &ctx, " eor<Enter>");
        let Figure::Circuit(c) = &a.fig else { panic!() };
        assert_eq!(c.elements[0].rot, 90, "r turns a point part while placing");
        assert_eq!(a.undo.len(), 1);
    }

    #[test]
    fn magnetic_flip_and_turns() {
        let fig = presets::build_named("transformer").unwrap();
        let (mut a, ctx) = app(fig);
        keys(&mut a, &ctx, "<Tab><Tab><Tab>F++");
        let Figure::Magnetic(m) = &a.fig else { panic!() };
        assert_eq!(m.windings[1].sense, "ccw");
        assert_eq!(m.windings[1].turns, 5);
    }
}
