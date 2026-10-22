//! Built-in presets: named figures for recurring lecture drawings. A preset is either a
//! generator call (so it stays regenerable) or a stored figure.

use serde_json::{Map, Value};

use crate::generators;
use crate::model::{Error, Figure};

pub enum Def {
    Gen { generator: &'static str, expr: &'static str, options: &'static str },
    Stored(&'static str),
}

pub struct Preset {
    pub name: &'static str,
    pub title: &'static str,
    pub def: Def,
}

pub const PRESETS: &[Preset] = &[
    Preset { name: "transformer", title: "Transformer", def: Def::Gen { generator: "magnetic", expr: "N_1 : N_2", options: "{}" } },
    Preset {
        name: "ideal-transformer",
        title: "Ideal transformer (split fluxes)",
        def: Def::Gen { generator: "magnetic", expr: "N_1 : N_2", options: r#"{"fluxes": "split", "ideal": true}"# },
    },
    Preset {
        name: "magnetic-circuit",
        title: "Magnetic circuit with air gap",
        def: Def::Gen { generator: "magnetic", expr: "N", options: r#"{"gap": "delta"}"# },
    },
    Preset { name: "voltage-divider", title: "Voltage divider", def: Def::Stored(include_str!("presets/voltage-divider.json")) },
    Preset { name: "rc-lowpass", title: "RC low-pass", def: Def::Stored(include_str!("presets/rc-lowpass.json")) },
    Preset { name: "inverting-amp", title: "Inverting amplifier", def: Def::Stored(include_str!("presets/inverting-amp.json")) },
    Preset {
        name: "mass-spring-damper",
        title: "Mass-spring-damper (ODE)",
        def: Def::Gen { generator: "ode", expr: "m y'' + d y' + c y = F", options: "{}" },
    },
];

pub fn find(name: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.name == name)
}

pub fn build(p: &Preset) -> Result<Figure, Error> {
    match &p.def {
        Def::Gen { generator, expr, options } => {
            let opts: Map<String, Value> = serde_json::from_str(options).expect("preset options are valid JSON");
            generators::generate(generator, expr, &opts)
        }
        Def::Stored(json) => Figure::from_json(json),
    }
}

pub fn build_named(name: &str) -> Result<Figure, Error> {
    let p = find(name).ok_or_else(|| {
        let names: Vec<_> = PRESETS.iter().map(|p| p.name).collect();
        Error(format!("unknown preset '{name}' (built-in: {})", names.join(", ")))
    })?;
    build(p)
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_preset_builds() {
        for p in super::PRESETS {
            super::build(p).unwrap_or_else(|e| panic!("{}: {e}", p.name));
        }
    }
}
