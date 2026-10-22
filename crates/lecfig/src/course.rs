//! Course folders: the `lecfig.toml` marker, `template.typ`, `figs/` and lecture files.

use std::fs;
use std::path::{Path, PathBuf};

use lecfig_core::Figure;

pub const MARKER: &str = "lecfig.toml";
const TEMPLATE: &str = include_str!("../../../templates/course/template.typ");
const MARKER_TEXT: &str = include_str!("../../../templates/course/lecfig.toml");

/// Walks up from `start` (a file or folder) to the folder holding `lecfig.toml`.
pub fn find_root(start: &Path) -> Result<PathBuf, String> {
    let start = if start.as_os_str().is_empty() { Path::new(".") } else { start };
    let abs = fs::canonicalize(start)
        .or_else(|_| start.parent().map(fs::canonicalize).unwrap_or(Err(std::io::ErrorKind::NotFound.into())))
        .map_err(|e| format!("{}: {e}", start.display()))?;
    let mut dir = if abs.is_dir() { abs.clone() } else { abs.parent().unwrap_or(&abs).to_path_buf() };
    loop {
        if dir.join(MARKER).is_file() {
            return Ok(dir);
        }
        if !dir.pop() {
            return Err(format!(
                "no {MARKER} found in {} or above; create a course with `lecfig course init <folder>`",
                abs.display()
            ));
        }
    }
}

/// Where a command looks for the course: next to `--for FILE`, else the working folder.
pub fn root_for(for_file: Option<&Path>) -> Result<PathBuf, String> {
    match for_file {
        // Helix passes "[scratch]" for unsaved buffers; fall back to the working folder.
        Some(f) if !f.as_os_str().is_empty() && !f.to_string_lossy().starts_with('[') => find_root(f),
        _ => find_root(Path::new(".")),
    }
}

pub fn init(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir.join("figs")).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (name, text) in [(MARKER, MARKER_TEXT), ("template.typ", TEMPLATE)] {
        let p = dir.join(name);
        if p.exists() {
            eprintln!("kept existing {}", p.display());
        } else {
            fs::write(&p, text).map_err(|e| format!("{}: {e}", p.display()))?;
        }
    }
    Ok(())
}

/// Creates today's lecture file next to `--for FILE` (or in the course root).
pub fn lecture(for_file: Option<&Path>) -> Result<PathBuf, String> {
    let root = root_for(for_file)?;
    let dir = match for_file {
        Some(f) if f.parent().is_some_and(|p| !p.as_os_str().is_empty()) => {
            fs::canonicalize(f.parent().unwrap()).map_err(|e| e.to_string())?
        }
        _ => root.clone(),
    };
    let today = typst_kit::datetime::Time::system().today(None).ok_or("no system date")?;
    let date = format!(
        "{:04}-{:02}-{:02}",
        today.year().unwrap_or(1970),
        today.month().unwrap_or(1),
        today.day().unwrap_or(1)
    );
    let path = dir.join(format!("{date}.typ"));
    if !path.exists() {
        let import = relative(&dir, &root.join("template.typ"));
        fs::write(&path, format!("#import \"{import}\": *\n\n= Lecture {date}\n\n"))
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(path)
}

/// Path of `target` as seen from folder `from`, with `/` separators.
fn relative(from: &Path, target: &Path) -> String {
    let f: Vec<_> = from.components().collect();
    let t: Vec<_> = target.components().collect();
    let common = f.iter().zip(&t).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = vec!["..".into(); f.len() - common];
    parts.extend(t[common..].iter().map(|c| c.as_os_str().to_string_lossy().into_owned()));
    parts.join("/")
}

pub fn figs_dir(root: &Path) -> PathBuf {
    root.join("figs")
}

/// Next free automatic name: fig-01, fig-02, …
pub fn next_name(root: &Path) -> String {
    let mut max = 0;
    if let Ok(entries) = fs::read_dir(figs_dir(root)) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if let Some(n) = name.strip_prefix("fig-").and_then(|s| s.strip_suffix(".json")).and_then(|s| s.parse::<u32>().ok()) {
                max = max.max(n);
            }
        }
    }
    format!("fig-{:02}", max + 1)
}

pub fn check_name(name: &str) -> Result<(), String> {
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err(format!("figure name '{name}' may only use letters, digits, '-' and '_'"));
    }
    Ok(())
}

/// Writes a figure atomically: a temporary file in the same folder, then a rename, so
/// tinymist and sync clients never read a half-written file.
pub fn write_figure(path: &Path, fig: &Figure) -> Result<(), String> {
    let dir = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let tmp = dir.join(format!(".{}.tmp", path.file_name().unwrap_or_default().to_string_lossy()));
    fs::write(&tmp, fig.to_json()).map_err(|e| format!("{}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn read_figure(path: &Path) -> Result<Figure, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Figure::from_json(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// A figure argument: a path to a .json file, or a name in the course's figs/.
pub fn resolve_figure(arg: &str, for_file: Option<&Path>) -> Result<PathBuf, String> {
    if arg.ends_with(".json") || arg.contains('/') {
        return Ok(PathBuf::from(arg));
    }
    Ok(figs_dir(&root_for(for_file)?).join(format!("{arg}.json")))
}

/// Renames figs/<old>.json and rewrites `#fig("old"` and `@fig:old` in the course's .typ
/// files. Returns the files that changed.
pub fn rename(root: &Path, old: &str, new: &str) -> Result<Vec<PathBuf>, String> {
    check_name(new)?;
    let (from, to) = (figs_dir(root).join(format!("{old}.json")), figs_dir(root).join(format!("{new}.json")));
    if !from.exists() {
        return Err(format!("{} does not exist", from.display()));
    }
    if to.exists() {
        return Err(format!("{} already exists", to.display()));
    }
    fs::rename(&from, &to).map_err(|e| e.to_string())?;
    let mut changed = vec![];
    for file in typ_files(root) {
        let Ok(text) = fs::read_to_string(&file) else { continue };
        let updated = rewrite_refs(&text, old, new);
        if updated != text {
            fs::write(&file, updated).map_err(|e| format!("{}: {e}", file.display()))?;
            changed.push(file);
        }
    }
    Ok(changed)
}

fn rewrite_refs(text: &str, old: &str, new: &str) -> String {
    let text = text.replace(&format!("fig(\"{old}\""), &format!("fig(\"{new}\""));
    // @fig:old, but not @fig:older
    let pat = format!("@fig:{old}");
    let mut out = String::new();
    let mut rest = text.as_str();
    while let Some(i) = rest.find(&pat) {
        let after = &rest[i + pat.len()..];
        let boundary = !after.starts_with(|c: char| c.is_alphanumeric() || c == '-' || c == '_');
        out.push_str(&rest[..i]);
        let replacement = if boundary { format!("@fig:{new}") } else { pat.clone() };
        out.push_str(&replacement);
        rest = after;
    }
    out.push_str(rest);
    out
}

fn typ_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    let Ok(entries) = fs::read_dir(dir) else { return out };
    for e in entries.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            out.extend(typ_files(&p));
        } else if p.extension().is_some_and(|x| x == "typ") {
            out.push(p);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_only_whole_references() {
        let t = "#fig(\"rc\")\nSee @fig:rc and @fig:rc-2.\n#fig(\"rc\", caption: [x])";
        assert_eq!(rewrite_refs(t, "rc", "lowpass"), "#fig(\"lowpass\")\nSee @fig:lowpass and @fig:rc-2.\n#fig(\"lowpass\", caption: [x])");
    }

    #[test]
    fn relative_import_paths() {
        assert_eq!(relative(Path::new("/c"), Path::new("/c/template.typ")), "template.typ");
        assert_eq!(relative(Path::new("/c/w1"), Path::new("/c/template.typ")), "../template.typ");
    }
}
