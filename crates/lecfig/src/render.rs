//! Renders figures with the embedded Typst compiler, through the installed lecfig
//! package, so the editor shows exactly what the notes will show.
//!
//! The render runs in editor mode: the renderer emits invisible markers (`<lecfig-grid>`
//! at grid points (0, 0) and (1, 1), `<lecfig-node>` at block-diagram nodes) whose
//! positions in the laid-out page give the exact grid ↔ pixel mapping.

use std::any::Any;
use std::io::{self, Read};
use std::path::Path;

use typst::diag::{FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Duration, Label, Selector, Value};
use typst::introspection::{DocumentPosition, MetadataElem};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::{LazyHash, PicoStr, Scalar};
use typst::{Library, LibraryExt, World};
use typst_kit::datetime::Time;
use typst_kit::files::{FileLoader, FileStore, FsRoot, SystemFiles};
use typst_kit::fonts::{self, FontStore};
use typst_kit::packages::SystemPackages;
use typst_layout::PagedDocument;

/// Page around the figure; matches the margin the editor assumes for panning.
const MAIN: &str = r#"#import "@local/lecfig:0.1.0": render
#set page(width: auto, height: auto, margin: 0.8cm, fill: white)
#render(json("/fig.json"), editor: true)
"#;

/// Packages come from the local cache only; never block the editor on the network.
struct NoDownload;

impl typst_kit::downloader::Downloader for NoDownload {
    fn stream(&self, _key: &dyn Any, url: &str) -> io::Result<(Option<usize>, Box<dyn Read>)> {
        Err(io::Error::other(format!(
            "package not in the local cache ({url}); compile a document that uses it once with `typst compile`"
        )))
    }
}

struct Loader {
    main: FileId,
    fig: FileId,
    main_src: Bytes,
    json: Bytes,
    system: SystemFiles,
}

impl FileLoader for Loader {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.main {
            Ok(self.main_src.clone())
        } else if id == self.fig {
            Ok(self.json.clone())
        } else {
            self.system.load(id)
        }
    }
}

pub struct FigWorld {
    library: LazyHash<Library>,
    fonts: FontStore,
    files: FileStore<Loader>,
    main: FileId,
    now: Time,
}

fn file_id(path: &str) -> FileId {
    RootedPath::new(VirtualRoot::Project, VirtualPath::new(path).expect("valid path")).intern()
}

impl FigWorld {
    /// `root` is the project root for relative files (the figure's folder).
    pub fn new(root: &Path) -> Self {
        let mut fonts = FontStore::new();
        fonts.extend(fonts::system());
        fonts.extend(fonts::embedded());
        let main = file_id("/main.typ");
        let loader = Loader {
            main,
            fig: file_id("/fig.json"),
            main_src: Bytes::new(MAIN.as_bytes().to_vec()),
            json: Bytes::new(b"{}".to_vec()),
            system: SystemFiles::new(FsRoot::new(root.to_path_buf()), SystemPackages::new(NoDownload)),
        };
        FigWorld {
            library: LazyHash::new(Library::builder().build()),
            fonts,
            files: FileStore::new(loader),
            main,
            now: Time::system(),
        }
    }

    pub fn render(&mut self, json: &str, pixel_per_pt: f32) -> Result<Rendered, String> {
        self.files.loader_mut().json = Bytes::new(json.as_bytes().to_vec());
        self.files.reset();
        let doc = typst::compile::<PagedDocument>(&*self).output.map_err(|errs| describe(&errs))?;
        let page = doc.pages().first().ok_or("empty document")?;
        let opts = typst_render::RenderOptions { pixel_per_pt: Scalar::new(pixel_per_pt as f64), render_bleed: false };
        let pixmap = typst_render::render(page, &opts);
        let intro = doc.introspector();
        let positions = |name: &str| -> Vec<(Value, (f32, f32))> {
            let sel = Selector::Label(Label::new(PicoStr::intern(name)).unwrap());
            typst::introspection::Introspector::query(intro.as_ref(), &sel)
                .iter()
                .filter_map(|c| {
                    let loc = c.location()?;
                    let DocumentPosition::Paged(p) = typst::introspection::Introspector::position(intro.as_ref(), loc)? else {
                        return None;
                    };
                    let value = c.to_packed::<MetadataElem>().map(|m| m.value.clone()).unwrap_or(Value::None);
                    let px = (p.point.x.to_pt() as f32 * pixel_per_pt, p.point.y.to_pt() as f32 * pixel_per_pt);
                    Some((value, px))
                })
                .collect()
        };
        let grid = positions("lecfig-grid");
        let transform = match grid.as_slice() {
            [(_, a), (_, b)] => Some(GridTransform { origin: *a, unit: (b.0 - a.0, b.1 - a.1) }),
            _ => None,
        };
        let nodes = positions("lecfig-node")
            .into_iter()
            .filter_map(|(v, p)| match v {
                Value::Str(s) => Some((s.to_string(), p)),
                _ => None,
            })
            .collect();
        Ok(Rendered {
            width: pixmap.width(),
            height: pixmap.height(),
            rgba: pixmap.data().to_vec(),
            transform,
            nodes,
        })
    }
}

fn describe(errs: &[SourceDiagnostic]) -> String {
    errs.iter()
        .map(|e| {
            let mut s = e.message.to_string();
            for h in &e.hints {
                s.push_str(&format!(" (hint: {})", h.v));
            }
            s
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// Maps grid units to pixels: px = origin + (x · unit.0, y · unit.1); unit.1 < 0 (y up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridTransform {
    pub origin: (f32, f32),
    pub unit: (f32, f32),
}

impl GridTransform {
    pub fn to_grid(&self, px: f32, py: f32) -> (f64, f64) {
        (((px - self.origin.0) / self.unit.0) as f64, ((py - self.origin.1) / self.unit.1) as f64)
    }
}

pub struct Rendered {
    pub width: u32,
    pub height: u32,
    /// Premultiplied RGBA.
    pub rgba: Vec<u8>,
    /// Present for circuit and magnetic figures.
    pub transform: Option<GridTransform>,
    /// Block-diagram node centres in pixels, by node id.
    pub nodes: Vec<(String, (f32, f32))>,
}

impl World for FigWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }
    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }
    fn main(&self) -> FileId {
        self.main
    }
    fn source(&self, id: FileId) -> FileResult<Source> {
        self.files.source(id)
    }
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files.file(id)
    }
    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }
    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        self.now.today(offset)
    }
}
