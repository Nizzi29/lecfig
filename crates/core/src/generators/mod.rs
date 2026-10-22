//! Generators: an expression (plain text whose identifiers are Typst math) in, a figure
//! with layout out. Every generated figure records its `source`, so it can be regenerated.

use serde_json::{Map, Value};

use crate::model::{Error, Figure, Source};

pub mod blockalg;
pub mod logic;
pub mod magnetic;
pub mod ode;
pub mod sp;

pub const GENERATORS: &[&str] = &["sp", "ode", "logic", "magnetic", "loop"];

pub fn generate(generator: &str, expr: &str, options: &Map<String, Value>) -> Result<Figure, Error> {
    let mut fig = match generator {
        "sp" => sp::generate(expr, options)?,
        "ode" => ode::generate(expr, options)?,
        "logic" => logic::generate(expr, options)?,
        "magnetic" => magnetic::generate(expr, options)?,
        "loop" => blockalg::generate(expr, options)?,
        other => {
            return Err(Error(format!("unknown generator '{other}' (known: {})", GENERATORS.join(", "))));
        }
    };
    let mut opts = options.clone();
    // Generators may add derived data (e.g. the ODE's rearranged equation) to the options.
    if let Some(src) = fig_source_mut(&mut fig).take() {
        for (k, v) in src.options {
            opts.entry(k).or_insert(v);
        }
    }
    *fig_source_mut(&mut fig) = Some(Source { generator: generator.into(), expr: expr.into(), options: opts });
    Ok(fig)
}

fn fig_source_mut(fig: &mut Figure) -> &mut Option<Source> {
    match fig {
        Figure::Circuit(c) => &mut c.source,
        Figure::Block(b) => &mut b.source,
        Figure::Magnetic(m) => &mut m.source,
    }
}

// ---------------------------------------------------------------- option helpers

pub(crate) fn opt_str<'a>(o: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    o.get(key).and_then(Value::as_str)
}

pub(crate) fn opt_bool(o: &Map<String, Value>, key: &str) -> bool {
    o.get(key).and_then(Value::as_bool).unwrap_or(false)
}

pub(crate) fn err<T>(msg: impl Into<String>) -> Result<T, Error> {
    Err(Error(msg.into()))
}

/// The subscript of a Typst math identifier: `N_1` → `1`, `N_(p 1)` → `(p 1)`, `N` → None.
pub(crate) fn subscript(ident: &str) -> Option<&str> {
    let i = ident.find('_')?;
    let s = ident[i + 1..].trim();
    (!s.is_empty()).then_some(s)
}

/// Joins a base and a subscript as Typst math: `u` + `1` → `u_1`; a missing subscript → `u`.
pub(crate) fn with_sub(base: &str, sub: Option<&str>) -> String {
    match sub {
        Some(s) => format!("{base}_{s}"),
        None => base.to_string(),
    }
}
