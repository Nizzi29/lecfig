//! lecfig: command-line tool and figure editor.

mod course;
mod editor;
mod render;
mod user_presets;

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use lecfig_core::model::{Block, Circuit, Figure};
use lecfig_core::{generators, presets};

#[derive(Parser)]
#[command(name = "lecfig", version, about = "Circuits, block diagrams and magnetic figures for Typst lecture notes")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Course folders.
    Course {
        #[command(subcommand)]
        cmd: CourseCmd,
    },
    /// Create today's lecture file (YYYY-MM-DD.typ) and print its path.
    Lecture {
        /// A file in the course; the lecture goes next to it.
        #[arg(long = "for")]
        for_file: Option<PathBuf>,
    },
    /// Create a figure, open the editor on it and print the line for the notes.
    New {
        /// Figure name; default: the next free fig-NN.
        name: Option<String>,
        /// The lecture file the figure is for (finds the course).
        #[arg(long = "for")]
        for_file: Option<PathBuf>,
        /// circuit | block (empty figures).
        #[arg(long, default_value = "circuit")]
        kind: String,
        /// network | equivalent | schematic | logic: the editor's first-level picker.
        #[arg(long)]
        flavor: Option<String>,
        /// Series/parallel network, e.g. "R_1 + (R_2 || C_1)".
        #[arg(long, conflicts_with_all = ["ode", "logic", "magnetic", "preset"])]
        sp: Option<String>,
        /// Linear ODE as block diagram, e.g. "m y'' + d y' + c y = F".
        #[arg(long, conflicts_with_all = ["logic", "magnetic", "preset"])]
        ode: Option<String>,
        /// Boolean expression, e.g. "y = (a and b) or not c".
        #[arg(long, conflicts_with_all = ["magnetic", "preset"])]
        logic: Option<String>,
        /// Transformer or magnetic circuit, e.g. "N_1 : N_2".
        #[arg(long, conflicts_with_all = ["preset", "loop_"])]
        magnetic: Option<String>,
        /// Control loop from block algebra, e.g. "feedback(C G, H)".
        #[arg(long = "loop", conflicts_with = "preset")]
        loop_: Option<String>,
        /// A named preset (see `lecfig presets`).
        #[arg(long)]
        preset: Option<String>,
        /// Generator option KEY=VALUE (VALUE as JSON if it parses, else text); repeatable.
        #[arg(long = "opt", value_name = "KEY=VALUE")]
        opts: Vec<String>,
        /// Do not open the editor.
        #[arg(long)]
        no_edit: bool,
        /// Overwrite an existing figure.
        #[arg(long)]
        force: bool,
    },
    /// Open the editor on an existing figure (name in the course, or a .json path).
    Edit {
        /// Figure name or path; or use --line to take the #fig("…") on a line of --for.
        figure: Option<String>,
        #[arg(long = "for")]
        for_file: Option<PathBuf>,
        /// Line (1-based) of --for holding #fig("name"); for editor key bindings.
        #[arg(long)]
        line: Option<usize>,
        /// Run in the foreground instead of detaching.
        #[arg(long)]
        wait: bool,
    },
    /// Rename a figure and update #fig("…") and @fig:… references in the course.
    Mv {
        old: String,
        new: String,
        #[arg(long = "for")]
        for_file: Option<PathBuf>,
    },
    /// List presets (built-in and yours), or save a figure as one of yours.
    Presets {
        #[command(subcommand)]
        cmd: Option<PresetCmd>,
    },
    /// Run the editor in the foreground (used by `new` and `edit`).
    #[command(hide = true)]
    Editor { path: PathBuf },
    /// Render a figure file to PNG with the embedded Typst (for checking).
    Png { figure: PathBuf, out: PathBuf },
}

#[derive(Subcommand)]
enum PresetCmd {
    /// Save a figure (name in the course, or a .json path) as preset NAME.
    Save {
        name: String,
        figure: String,
        #[arg(long = "for")]
        for_file: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum CourseCmd {
    /// Create a course: lecfig.toml, template.typ and figs/.
    Init { dir: PathBuf },
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.cmd {
        Cmd::Course { cmd: CourseCmd::Init { dir } } => course::init(&dir).map(|()| {
            println!("course ready in {}; lecture files start with #import \"template.typ\": *", dir.display());
        }),
        Cmd::Lecture { for_file } => course::lecture(for_file.as_deref()).map(|p| println!("{}", p.display())),
        Cmd::New { name, for_file, kind, flavor, sp, ode, logic, magnetic, loop_, preset, opts, no_edit, force } => {
            let source = [("sp", sp), ("ode", ode), ("logic", logic), ("magnetic", magnetic), ("loop", loop_)]
                .into_iter()
                .find_map(|(g, e)| e.map(|e| (g, e)));
            new(name, for_file.as_deref(), &kind, flavor, source, preset, &opts, no_edit, force)
        }
        Cmd::Edit { figure, for_file, line, wait } => figure_arg(figure, for_file.as_deref(), line)
            .and_then(|figure| course::resolve_figure(&figure, for_file.as_deref()))
            .and_then(|p| {
            if !p.exists() {
                return Err(format!("{} does not exist (create it with `lecfig new`)", p.display()));
            }
            if wait { editor::run(p) } else { spawn_editor(&p) }
        }),
        Cmd::Mv { old, new, for_file } => course::root_for(for_file.as_deref()).and_then(|root| {
            let changed = course::rename(&root, &old, &new)?;
            println!("renamed {old} → {new}; updated {} file(s)", changed.len());
            for f in changed {
                println!("  {}", f.display());
            }
            Ok(())
        }),
        Cmd::Presets { cmd: None } => {
            for p in presets::PRESETS {
                println!("{:<20} {}", p.name, p.title);
            }
            for n in user_presets::names() {
                println!("{n:<20} (yours, {})", user_presets::dir().display());
            }
            Ok(())
        }
        Cmd::Presets { cmd: Some(PresetCmd::Save { name, figure, for_file }) } => {
            course::resolve_figure(&figure, for_file.as_deref()).and_then(|p| user_presets::save(&name, &p)).map(|path| {
                println!("saved preset {name} ({})", path.display());
            })
        }
        Cmd::Editor { path } => editor::run(path),
        Cmd::Png { figure, out } => png(&figure, &out),
    };
    if let Err(e) = result {
        eprintln!("lecfig: {e}");
        std::process::exit(1);
    }
}

/// The figure to edit: given directly, or the #fig("…") on line `line` of `for_file`.
fn figure_arg(figure: Option<String>, for_file: Option<&Path>, line: Option<usize>) -> Result<String, String> {
    if let Some(f) = figure {
        return Ok(f);
    }
    let (Some(file), Some(line)) = (for_file, line) else {
        return Err("name a figure, or give --for FILE --line N".into());
    };
    let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let l = text.lines().nth(line.saturating_sub(1)).unwrap_or_default();
    let start = l.find("fig(\"").ok_or_else(|| format!("no #fig(\"…\") on line {line}"))? + 5;
    let end = l[start..].find('"').ok_or("unterminated figure name")? + start;
    Ok(l[start..end].to_string())
}

#[allow(clippy::too_many_arguments)]
fn new(
    name: Option<String>,
    for_file: Option<&Path>,
    kind: &str,
    flavor: Option<String>,
    source: Option<(&str, String)>,
    preset: Option<String>,
    opts: &[String],
    no_edit: bool,
    force: bool,
) -> Result<(), String> {
    let root = course::root_for(for_file)?;
    check_imports(for_file)?;
    let name = name.unwrap_or_else(|| course::next_name(&root));
    course::check_name(&name)?;
    let path = course::figs_dir(&root).join(format!("{name}.json"));
    if path.exists() && !force {
        return Err(format!("{} exists; open it with `lecfig edit {name}` or use --force", path.display()));
    }
    let mut options = serde_json::Map::new();
    for o in opts {
        let (k, v) = o.split_once('=').ok_or_else(|| format!("--opt '{o}' is not KEY=VALUE"))?;
        let value = serde_json::from_str(v).unwrap_or_else(|_| serde_json::Value::String(v.to_string()));
        options.insert(k.to_string(), value);
    }
    let fig = match (source, preset) {
        (Some((g, expr)), _) => generators::generate(g, &expr, &options).map_err(|e| e.0)?,
        (None, Some(p)) => user_presets::build(&p)?,
        (None, None) => match kind {
            "block" => Figure::Block(Block::default()),
            "circuit" => Figure::Circuit(Circuit { flavor: flavor.clone().unwrap_or_else(|| "network".into()), ..Default::default() }),
            other => return Err(format!("--kind must be circuit or block, got '{other}' (magnetic figures come from --magnetic or --preset)")),
        },
    };
    let fig = match (fig, flavor) {
        (Figure::Circuit(mut c), Some(f)) => {
            c.flavor = f;
            Figure::Circuit(c)
        }
        (fig, _) => fig,
    };
    course::write_figure(&path, &fig)?;
    if !no_edit {
        spawn_editor(&path)?;
    }
    // No newline: Helix's :insert-output puts this right at the cursor.
    print!("#fig(\"{name}\")");
    Ok(())
}

/// Where the detached editor writes what it prints (errors, panics).
fn editor_log() -> PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state"));
    base.join("lecfig").join("editor.log")
}

/// Starts the editor as its own process, detached, so the caller (Helix) is not blocked.
/// If it dies right away (no display, bad figure, …) the error is returned, so Helix
/// shows it instead of a silent "Command run".
fn spawn_editor(path: &Path) -> Result<(), String> {
    use std::process::{Command, Stdio};
    if std::env::var_os("WAYLAND_DISPLAY").is_none() && std::env::var_os("DISPLAY").is_none() {
        return Err("no graphical session here (WAYLAND_DISPLAY and DISPLAY are unset); \
             inside tmux/zellij the editor cannot open a window"
            .into());
    }
    let log = editor_log();
    if let Some(dir) = log.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let out = std::fs::File::create(&log).map_err(|e| format!("{}: {e}", log.display()))?;
    let err = out.try_clone().map_err(|e| e.to_string())?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd = Command::new(exe);
    cmd.arg("editor").arg(path).stdin(Stdio::null()).stdout(out).stderr(err);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd.spawn().map_err(|e| format!("could not start the editor: {e}"))?;
    // Window creation fails within a few hundred milliseconds if it fails at all.
    for _ in 0..8 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Ok(Some(status)) = child.try_wait() {
            if status.success() {
                return Ok(());
            }
            let text = std::fs::read_to_string(&log).unwrap_or_default();
            let last: Vec<&str> = text.lines().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect();
            return Err(format!("the editor stopped right away: {} (full log: {})", last.join(" | "), log.display()));
        }
    }
    Ok(())
}

/// Lecture files must import the course template, or `#fig("…")` is unknown to Typst.
fn check_imports(for_file: Option<&Path>) -> Result<(), String> {
    let Some(file) = for_file else { return Ok(()) };
    let Ok(text) = std::fs::read_to_string(file) else { return Ok(()) };
    if text.contains("template.typ") || text.contains("#let fig(") {
        return Ok(());
    }
    Err(format!(
        "{} does not import the course template, so #fig would be unknown; add `#import \"template.typ\": *` near the top and try again",
        file.display()
    ))
}

fn png(figure: &Path, out: &Path) -> Result<(), String> {
    let json = std::fs::read_to_string(figure).map_err(|e| format!("{}: {e}", figure.display()))?;
    let dir = figure.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let mut world = render::FigWorld::new(&dir);
    let r = world.render(&json, 2.0)?;
    write_png(out, r.width, r.height, &r.rgba)
}

/// Minimal PNG writer (stored deflate blocks), so the check needs no image crate.
pub fn write_png(path: &Path, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    fn crc(data: &[u8]) -> u32 {
        let mut c = 0xffff_ffffu32;
        for &b in data {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
        out.extend((data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend(data);
        out.extend(&body);
        out.extend(crc(&body).to_be_bytes());
    }
    let mut raw = vec![];
    for row in rgba.chunks(w as usize * 4) {
        raw.push(0);
        raw.extend(row);
    }
    let mut z = vec![0x78, 0x01];
    for (i, block) in raw.chunks(65535).enumerate() {
        let last = (i + 1) * 65535 >= raw.len();
        z.push(last as u8);
        z.extend((block.len() as u16).to_le_bytes());
        z.extend((!(block.len() as u16)).to_le_bytes());
        z.extend(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend(((b << 16) | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = vec![];
    ihdr.extend(w.to_be_bytes());
    ihdr.extend(h.to_be_bytes());
    ihdr.extend([8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    std::fs::write(path, out).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn command_line_definition_is_consistent() {
        use clap::CommandFactory;
        super::Cli::command().debug_assert();
    }

    #[test]
    fn lecture_without_template_import_is_refused() {
        let dir = std::env::temp_dir().join("lecfig-import-test");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("l.typ");
        std::fs::write(&f, "= A\n").unwrap();
        assert!(super::check_imports(Some(&f)).is_err());
        std::fs::write(&f, "#import \"template.typ\": *\n").unwrap();
        assert!(super::check_imports(Some(&f)).is_ok());
    }

    #[test]
    fn figure_on_a_line() {
        let dir = std::env::temp_dir().join("lecfig-line-test");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("l.typ");
        std::fs::write(&f, "= A\n#fig(\"rc\", caption: [x])\ntext\n").unwrap();
        assert_eq!(super::figure_arg(None, Some(&f), Some(2)).unwrap(), "rc");
        assert!(super::figure_arg(None, Some(&f), Some(3)).is_err());
        assert_eq!(super::figure_arg(Some("x".into()), None, None).unwrap(), "x");
    }
}
