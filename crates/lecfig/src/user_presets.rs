//! Your own presets: figure files in `$XDG_CONFIG_HOME/lecfig/presets/` (default
//! `~/.config/lecfig/presets/`). They come before built-in presets of the same name.

use std::path::{Path, PathBuf};

use lecfig_core::{Figure, presets};

pub fn dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
    base.join("lecfig").join("presets")
}

/// Names of your presets, sorted.
pub fn names() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir())
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| e.file_name().to_string_lossy().strip_suffix(".json").map(String::from))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// A preset by name: yours first, then the built-in one.
pub fn build(name: &str) -> Result<Figure, String> {
    let path = dir().join(format!("{name}.json"));
    if path.exists() {
        return crate::course::read_figure(&path);
    }
    presets::build_named(name).map_err(|e| {
        let mine = names();
        if mine.is_empty() { e.0 } else { format!("{}; yours: {}", e.0, mine.join(", ")) }
    })
}

pub fn save(name: &str, figure: &Path) -> Result<PathBuf, String> {
    crate::course::check_name(name)?;
    let fig = crate::course::read_figure(figure)?;
    let path = dir().join(format!("{name}.json"));
    crate::course::write_figure(&path, &fig)?;
    Ok(path)
}
