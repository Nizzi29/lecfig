//! lecfig-core: the figure data model, the part catalog and the generators.
//!
//! Everything here is plain Rust without file or network access, so the same
//! code runs in the `lecfig` binary and inside Typst as a WebAssembly plugin.

/// Version of the figure JSON format this crate reads and writes.
pub const SCHEMA_VERSION: u32 = 1;

pub mod generators;
pub mod model;
pub mod parts;
pub mod presets;

pub use model::{Figure, Pt};

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
  "kind": "circuit",
  "version": 1,
  "flavor": "network",
  "elements": [
    {"id": "v1", "type": "vsource", "from": [0, 0], "to": [0, 3], "label": "U_0", "u": "u_1"},
    {"id": "r1", "type": "resistor", "from": [0, 3], "to": [3, 3], "label": "R_1", "i": {"content": "i", "invert": true}},
    {"id": "c1", "type": "capacitor", "from": [3, 3], "to": [3, 0], "label": "C"},
    {"id": "g1", "type": "land", "at": [6, 1.5], "rot": 90, "inputs": 3},
    {"id": "gnd", "type": "ground", "at": [0, 0]}
  ],
  "wires": [
    {"points": ["c1.out", [1.5, 0], "v1.in"]}
  ],
  "annotations": [
    {"type": "voltage", "from": "r1.in", "to": [3, 0], "content": "U_(a b)"},
    {"type": "callout", "to": "r1.out", "rel": [1, 1], "content": "R_1 = 0"}
  ]
}
"#;

    #[test]
    fn round_trip_is_lossless_and_stable() {
        let fig = Figure::from_json(SAMPLE).unwrap();
        let text = fig.to_json();
        assert_eq!(Figure::from_json(&text).unwrap(), fig);
        assert_eq!(Figure::from_json(&text).unwrap().to_json(), text);
        assert_eq!(text, SAMPLE, "writer keeps the sample's layout");
    }

    #[test]
    fn rejects_newer_schema() {
        let text = SAMPLE.replace("\"version\": 1", "\"version\": 99");
        assert!(Figure::from_json(&text).is_err());
    }
}
