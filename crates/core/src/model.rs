//! The figure data model: one JSON file per figure, `kind` selects the variant.
//!
//! Field names match the JSON keys the Typst renderer reads (`typst/lecfig/render-*.typ`).
//! Unknown keys on elements, nodes and edges are kept in `extra`, so files written by a
//! newer version survive a round trip through an older editor.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::SCHEMA_VERSION;

/// A point in grid units (circuits: 1 = 1 cm, y up; block diagrams: fletcher cells, y down).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Pt(pub f64, pub f64);

impl Pt {
    pub fn new(x: f64, y: f64) -> Self {
        Pt(x, y)
    }
    pub fn add(self, o: Pt) -> Pt {
        Pt(self.0 + o.0, self.1 + o.1)
    }
    pub fn sub(self, o: Pt) -> Pt {
        Pt(self.0 - o.0, self.1 - o.1)
    }
    pub fn scale(self, k: f64) -> Pt {
        Pt(self.0 * k, self.1 * k)
    }
    pub fn dist(self, o: Pt) -> f64 {
        ((self.0 - o.0).powi(2) + (self.1 - o.1).powi(2)).sqrt()
    }
    /// Rotates by a multiple of 90° counter-clockwise (exact, no float drift).
    pub fn rot90(self, deg: i32) -> Pt {
        match deg.rem_euclid(360) {
            90 => Pt(-self.1, self.0),
            180 => Pt(-self.0, -self.1),
            270 => Pt(self.1, -self.0),
            _ => self,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Figure {
    Circuit(Circuit),
    Block(Block),
    Magnetic(Magnetic),
}

/// Where a generated figure came from, so it can be regenerated.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Source {
    pub generator: String,
    pub expr: String,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub options: Map<String, Value>,
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

// ---------------------------------------------------------------- circuits

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Circuit {
    pub version: u32,
    /// network | equivalent | schematic | logic: decides the editor's part picker.
    #[serde(default = "default_flavor")]
    pub flavor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    #[serde(default)]
    pub elements: Vec<Element>,
    #[serde(default)]
    pub wires: Vec<Wire>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
}

fn default_flavor() -> String {
    "network".into()
}

impl Default for Circuit {
    fn default() -> Self {
        Circuit {
            version: SCHEMA_VERSION,
            flavor: default_flavor(),
            source: None,
            elements: vec![],
            wires: vec![],
            annotations: vec![],
        }
    }
}

/// One part. Two-terminal parts use `from`/`to` (their pins `in`/`out`); all others use
/// `at` + `rot` + `flip` with pins from the part catalog.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Element {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Pt>,
    /// Degrees counter-clockwise, a multiple of 90.
    #[serde(default, skip_serializing_if = "is_default")]
    pub rot: i32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub flip: bool,
    /// Typst math source.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// Voltage arrow along the part.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub u: Option<Deco>,
    /// Current arrow on the part's lead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub i: Option<Deco>,
    /// Type-specific options (`inputs` of a gate, `open` of a node, …) and unknown keys.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A voltage or current decoration: plain Typst math, or with options.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Deco {
    Text(String),
    Full {
        content: String,
        #[serde(default, skip_serializing_if = "is_default")]
        invert: bool,
        /// Side of the part: north | south | east | west (zap's anchor names).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        anchor: Option<String>,
    },
}

impl Deco {
    pub fn content(&self) -> &str {
        match self {
            Deco::Text(s) => s,
            Deco::Full { content, .. } => content,
        }
    }
    pub fn invert(&self) -> bool {
        matches!(self, Deco::Full { invert: true, .. })
    }
    pub fn with_invert(&self, invert: bool) -> Deco {
        match self {
            Deco::Text(s) if !invert => Deco::Text(s.clone()),
            Deco::Text(s) => Deco::Full { content: s.clone(), invert, anchor: None },
            Deco::Full { content, anchor, .. } if !invert && anchor.is_none() => {
                Deco::Text(content.clone())
            }
            Deco::Full { content, anchor, .. } => {
                Deco::Full { content: content.clone(), invert, anchor: anchor.clone() }
            }
        }
    }
}

/// A wire: a polyline whose points are grid points or pin references (`"r1.out"`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Wire {
    pub points: Vec<WirePt>,
    /// Current arrow in the middle of the wire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub i: Option<Deco>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WirePt {
    Pin(String),
    At(Pt),
}

/// Free annotations, shared by all kinds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Annotation {
    /// Text at a point.
    Text {
        at: Pt,
        content: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tone: Option<String>,
    },
    /// Voltage arrow between two points or pins (e.g. U_ab between terminals).
    Voltage {
        from: Target,
        to: Target,
        content: String,
        #[serde(default, skip_serializing_if = "is_default")]
        invert: bool,
        /// Bulge of the arrow to the left of from→to, in grid units (negative = right).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bend: Option<f64>,
    },
    /// Text placed `rel` away from its target, with an arrow to the target.
    Callout {
        to: Target,
        rel: Pt,
        content: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tone: Option<String>,
    },
}

/// What an annotation points at.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Target {
    /// A pin or anchor name: `"r1.out"`, `"w1.lead2"`, `"s1.north"`.
    Anchor(String),
    At(Pt),
    /// A point on a magnetic core.
    Core(CorePt),
}

// ---------------------------------------------------------------- block diagrams

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub version: u32,
    #[serde(default)]
    pub style: BlockStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    #[serde(default)]
    pub nodes: Vec<BNode>,
    #[serde(default)]
    pub edges: Vec<BEdge>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
}

impl Default for Block {
    fn default() -> Self {
        Block {
            version: SCHEMA_VERSION,
            style: BlockStyle::default(),
            source: None,
            nodes: vec![],
            edges: vec![],
            annotations: vec![],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlockStyle {
    /// laplace (1/s, G(s)) | time (∫)
    pub blocks: String,
}

impl Default for BlockStyle {
    fn default() -> Self {
        BlockStyle { blocks: "laplace".into() }
    }
}

/// A block-diagram node. Types: sum, gain, integrator, tf, function, io, branch.
/// `at` is a fletcher cell (column, row), rows grow downwards.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BNode {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub at: Pt,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A signal line. `sign` is drawn at the arrow head (for sums); `bends` are cells the
/// line passes through; `label` names the signal.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BEdge {
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sign: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bends: Vec<Pt>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

// ---------------------------------------------------------------- magnetic figures

/// An iron core in section with windings, fluxes, cross-sections and callouts.
/// Positions are on the core: see [`CorePt`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Magnetic {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<MagStyle>,
    pub core: Core,
    #[serde(default)]
    pub windings: Vec<Winding>,
    #[serde(default)]
    pub fluxes: Vec<Flux>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<Section>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MagStyle {
    /// lecture (electrical red, magnetic green) | mono
    pub colors: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Core {
    /// ring (two legs, two yokes)
    pub shape: String,
    /// Outer width and height in cm.
    pub size: Pt,
    /// Thickness of the legs (left/right) and yokes (top/bottom) in cm.
    pub leg: f64,
    pub yoke: f64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(rename = "label-at", default, skip_serializing_if = "Option::is_none")]
    pub label_at: Option<CorePt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gaps: Vec<Gap>,
}

/// A point on the core: `leg` left | right | top | bottom, `pos` 0..1 along the window
/// edge (bottom→top on legs, left→right on yokes), `lane` −1 window edge … 1 outer edge.
/// With `flux`, the lane is taken from that flux.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CorePt {
    pub leg: String,
    pub pos: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flux: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gap {
    pub leg: String,
    pub pos: f64,
    pub width: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Winding {
    pub id: String,
    pub leg: String,
    /// cw | ccw: current into terminal 1 drives a clockwise / counter-clockwise flux.
    #[serde(default = "cw")]
    pub sense: String,
    /// Turns drawn (the label carries the real number).
    #[serde(default = "three")]
    pub turns: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// Terminal 1 and 2 positions along the leg.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Pt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<Port>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn cw() -> String {
    "cw".into()
}
fn three() -> u32 {
    3
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Port {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub i: Option<String>,
    /// in | out (relative to terminal 1)
    #[serde(rename = "i-dir", default, skip_serializing_if = "Option::is_none")]
    pub i_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub u: Option<String>,
    /// 1-2 | 2-1
    #[serde(rename = "u-dir", default, skip_serializing_if = "Option::is_none")]
    pub u_dir: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Flux {
    pub id: String,
    /// loop (closed path around the core) or a leg name (straight arrow on that leg).
    pub path: String,
    pub lane: f64,
    #[serde(default = "cw")]
    pub dir: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arrows: Vec<CorePt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Pt>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub id: String,
    pub leg: String,
    pub pos: f64,
}

// ---------------------------------------------------------------- reading and writing

#[derive(Debug)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl Figure {
    pub fn kind(&self) -> &'static str {
        match self {
            Figure::Circuit(_) => "circuit",
            Figure::Block(_) => "block",
            Figure::Magnetic(_) => "magnetic",
        }
    }

    pub fn source(&self) -> Option<&Source> {
        match self {
            Figure::Circuit(c) => c.source.as_ref(),
            Figure::Block(b) => b.source.as_ref(),
            Figure::Magnetic(m) => m.source.as_ref(),
        }
    }

    pub fn from_json(text: &str) -> Result<Figure, Error> {
        let fig: Figure = serde_json::from_str(text).map_err(|e| Error(format!("figure JSON: {e}")))?;
        let version = match &fig {
            Figure::Circuit(c) => c.version,
            Figure::Block(b) => b.version,
            Figure::Magnetic(m) => m.version,
        };
        if version > SCHEMA_VERSION {
            return Err(Error(format!(
                "figure has schema version {version}, this lecfig reads up to {SCHEMA_VERSION}"
            )));
        }
        Ok(fig)
    }

    /// Pretty JSON that diffs well: one key per line at the top, one element per line in
    /// lists, everything deeper on one line. Whole numbers are written without `.0`.
    pub fn to_json(&self) -> String {
        let value = serde_json::to_value(self).expect("figures always serialize");
        let mut out = String::new();
        write_top(&normalize(value), &mut out);
        out.push('\n');
        out
    }
}

/// Writes whole floats as integers and rounds the rest to 4 decimals.
fn normalize(v: Value) -> Value {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() => {
                let r = (f * 10000.0).round() / 10000.0;
                if r.fract() == 0.0 && r.abs() < 1e15 {
                    Value::from(r as i64)
                } else {
                    Value::from(r)
                }
            }
            _ => Value::Number(n),
        },
        Value::Array(a) => Value::Array(a.into_iter().map(normalize).collect()),
        Value::Object(o) => Value::Object(o.into_iter().map(|(k, v)| (k, normalize(v))).collect()),
        other => other,
    }
}

fn write_top(v: &Value, out: &mut String) {
    let Value::Object(map) = v else {
        out.push_str(&compact(v));
        return;
    };
    out.push_str("{\n");
    let n = map.len();
    for (i, (k, v)) in map.iter().enumerate() {
        out.push_str("  ");
        out.push_str(&serde_json::to_string(k).unwrap());
        out.push_str(": ");
        match v {
            Value::Array(items) if items.iter().any(|x| x.is_object() || x.is_array()) => {
                out.push_str("[\n");
                for (j, item) in items.iter().enumerate() {
                    out.push_str("    ");
                    out.push_str(&compact(item));
                    if j + 1 < items.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                out.push_str("  ]");
            }
            _ => out.push_str(&compact(v)),
        }
        if i + 1 < n {
            out.push(',');
        }
        out.push('\n');
    }
    out.push('}');
}

/// One-line JSON with a space after `:` and `,` for readability.
fn compact(v: &Value) -> String {
    match v {
        Value::Array(a) => {
            let parts: Vec<String> = a.iter().map(compact).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Object(o) => {
            let parts: Vec<String> = o
                .iter()
                .map(|(k, v)| format!("{}: {}", serde_json::to_string(k).unwrap(), compact(v)))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
        other => serde_json::to_string(other).unwrap(),
    }
}
