//! Typst plugin: exposes lecfig-core to Typst as WebAssembly.
//!
//! Build with `just plugin`; the package loads it with `plugin("lecfig.wasm")`.
//! Typst passes arguments as bytes and expects bytes back; errors become Typst errors.

use lecfig_core::{generators, presets};
use wasm_minimal_protocol::*;

initiate_protocol!();

fn text(bytes: &[u8]) -> Result<&str, String> {
    std::str::from_utf8(bytes).map_err(|e| format!("lecfig: argument is not UTF-8: {e}"))
}

/// The schema version as text, so Typst can check it loaded a matching plugin.
#[wasm_func]
pub fn schema_version() -> Vec<u8> {
    lecfig_core::SCHEMA_VERSION.to_string().into_bytes()
}

/// generate(generator, expr, options as JSON) → figure JSON.
#[wasm_func]
pub fn generate(generator: &[u8], expr: &[u8], options: &[u8]) -> Result<Vec<u8>, String> {
    let options: serde_json::Map<String, serde_json::Value> =
        serde_json::from_slice(options).map_err(|e| format!("lecfig: options: {e}"))?;
    let fig = generators::generate(text(generator)?, text(expr)?, &options).map_err(|e| e.0)?;
    Ok(fig.to_json().into_bytes())
}

/// preset(name) → figure JSON.
#[wasm_func]
pub fn preset(name: &[u8]) -> Result<Vec<u8>, String> {
    let fig = presets::build_named(text(name)?).map_err(|e| e.0)?;
    Ok(fig.to_json().into_bytes())
}
