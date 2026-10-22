# lecfig — implementation plan

This is the canonical plan; the status section below tracks progress.

## Status (2026-10-16)

| Done | Next |
| --- | --- |
| M0 checks; data model with lossless JSON round trip; part catalog with pins probed from zap; renderers for circuit (zap), block (fletcher) and magnetic (cetz) figures; generators `sp`, `ode`, `logic`, `magnetic`, `loop` (block algebra); built-in presets and your own (`lecfig presets save`); Typst plugin + package API; CLI (`course init`, `lecture`, `new`, `edit`, `mv`, `presets`, `png`); editor with embedded Typst render, modal keys for circuits, block diagrams and magnetic figures, autosave, undo; Helix glue (`docs/helix.md`) | Use in lectures and fix what gets in the way; inserting a preset into a non-empty figure; rubber-band selection; freeze to plain zap/fletcher; magnetic backlog (leakage fluxes, three-leg cores) |

## Context

The goal: electrical circuits, logic circuits, block diagrams and magnetic figures in Typst lecture notes, fast enough to keep up with an EE lecture. The architecture: **figure data (JSON) is the single source of truth**; generators write it, a mouse/keyboard editor edits it, a Typst renderer draws it with zap/fletcher, and tinymist's preview shows the result live.

Decisions:

| Topic | Decision |
| --- | --- |
| Stack | **Rust everywhere** except the renderer (Typst, unavoidable). Rust core shared by a Typst WASM plugin and a native egui editor. Route A (CircuiTikZ converter) and C (Hilbert) dropped. |
| Generators | In the Rust core; exposed to Typst via WASM plugin (`#logic`, `#sp`, …) and to the editor directly. Inputs are plain strings whose identifiers are Typst math (`R_1 \|\| (R_3 + C_"load")`), so generated labels are valid Typst math. |
| Storage | One JSON per figure in `<course>/figs/`. Notes reference it as `#fig("rc-filter")` via a wrapper in the course template (packages cannot read user files). |
| Diagram types | All equal: schematic, lumped-element network, equivalent circuit, logic diagram (all `kind: "circuit"`, zap), block diagram (`kind: "block"`, fletcher) and magnetic figure (`kind: "magnetic"`: iron core, windings, fluxes; drawn with cetz because zap has no core symbols). Figure stores its `flavor`; the picker adapts to it. |
| Presets | Recurring figures (ideal transformer, voltage divider, RC low-pass …) are named generator calls, reachable three ways: the Typst call, `lecfig new --preset`, and a preset picker in the editor. A figure made from one keeps its `source`, so it can be regenerated. |
| Symbols | IEC/DIN throughout (zap `variant: "iec"`). Block style switchable per figure: Laplace (`1/s`, `G(s)`) or time domain (`∫`). |
| Editor canvas | **Real render**: editor embeds the Typst compiler and shows actual zap/fletcher output; draws only cursor, pins, selection on top. |
| Editor input | Helix-like modal keymap (draft below), mouse for click-select/drag. Keymap in a config file. |
| Labels | Auto-numbered for hand-placed parts (R_1, R_2 …, continuing after generated ones); generated parts take the identifier from the expression. Add/edit/delete in editor. |
| Figures | Numbered (`figure`) with optional caption, label `<fig:name>` for `@fig:name`. |
| Naming | Both: one Helix key creates auto-named `fig-07`; `lecfig new <name>` when there's time; `lecfig mv` renames. |
| Notes layout | File per lecture inside a course folder; location anywhere. Tool assumes no paths. |
| Name | `lecfig` for now; the name stays out of figure JSON and lecture files, so a rename touches only this repo, each course's `template.typ` import, and the Helix keybinding. |
| Pace | As fast as possible, for use in lectures. Order: Typst-side figures and generators first (usable in notes without the editor), then CLI, then editor. |
| ODE generator | Flagship generator: full block diagram from a linear ODE of any order. Nonlinear terms, input derivatives (canonical forms), coupled systems tracked as backlog. |
| Portability | Generic, no hardcoded paths, XDG config (`~/.config/lecfig/`), Helix glue optional and documented, Typst package kept Universe-compatible. |

## Environment facts (checked 2026-10-13, M0 results 2026-10-16)

- Typst CLI 0.15.1 and `just` installed; the Rust → WebAssembly → Typst plugin chain works.
- Preview refresh: tinymist's preview pushes a new frame after every JSON write, for in-place writes and for temp-file + rename (checked with an LSP harness using the Helix config). Delays seen when editing the JSON by hand in a second editor come from the watcher: tinymist drops a file watch after some rename patterns and re-arms it on the next compile, which saving the `.typ` triggers.
- zap 0.6.0's `wire(i: …)` panics; lecfig draws wire currents itself (`common.typ`).

- tinymist in `~/.cargo/bin`, built on **Typst 0.15.1**. **Typst CLI not installed.** No Typst packages cached yet (zap/fletcher never downloaded here).
- Rust 1.97.1 + cargo, Node 24, Python 3.14. Helix 25.07.1 with tinymist's background preview.
- zap docs list: resistor, capacitor, inductor, vsource, isource, battery, diode, bjt, mosfet, jfet, opamp, gates `land/lor/lxor/lnot` (+`invert`), flip-flops, ground/earth/vcc, voltmeter/ammeter, `node`, `wire`/`swire`/`zwire`, `u:`/`i:` decorations, anchors `in`/`out`/`inN`. Default variant `iec`. M0 found controlled sources too (`dvsource`, `disource`), the AC source (`current: "ac"`), and the IEC box doubles as impedance Z, so equivalent circuits need no custom symbols.
- zap's `transformer` is only the two-circle schematic symbol; it has nothing for cores, windings or flux paths. Magnetic figures are therefore drawn with cetz 0.5.2, the version zap already pins, so no new dependency. The spike `tests/magnetic/transformer.typ` compiles on Typst 0.15.1 (checked 2026-10-14).

## Repository layout

```
lecfig/
  Cargo.toml                 workspace
  justfile                   build, install, test recipes
  crates/
    core/                    lecfig-core: data model (serde), part catalog + pins,
                             parsers + layout (logic, sp, ode, blockalg, magnetic),
                             built-in presets, id/label numbering
    plugin/                  lecfig-plugin: cdylib → wasm32-unknown-unknown, wasm-minimal-protocol;
                             thin wrappers: expr string in → figure JSON out
    lecfig/                  binary: CLI (clap) + editor (eframe/egui) + embedded Typst
                             (typst, typst-render, typst-kit for fonts + package storage)
  typst/lecfig/              the Typst package
    typst.toml
    lib.typ                  public API: fig, render, logic, sp, ode, loop
    render-circuit.typ       JSON → zap (ONLY place zap is called)
    render-block.typ         JSON → fletcher (ONLY place fletcher is called)
    render-magnetic.typ      JSON → cetz: cores, windings, fluxes (spike exists)
    symbols.typ              custom cetz symbols zap lacks (controlled sources, Z box)
    lecfig.wasm              built from crates/plugin (gitignored, built by `just`)
  templates/course/          what `lecfig course init` copies: template.typ, figs/, lecture stub
  probe/pins.typ             Typst probe that dumps zap anchor positions → core/pins.json
  tests/                     fixture figures + expected outputs
  docs/                      schema.md, keymap.md, helix.md
```

`just install` symlinks `typst/lecfig` → `~/.local/share/typst/packages/local/lecfig/0.1.0` (XDG data dir) and runs `cargo install --path crates/lecfig`.

## Data model v1

```json
{
  "version": 1,
  "kind": "circuit",
  "flavor": "network",
  "style": { "variant": "iec" },
  "source": { "generator": "sp", "expr": "R_1 || (R_3 + C_1)" },
  "elements": [
    { "id": "r1", "type": "resistor", "from": [0, 0], "to": [2, 0], "label": "R_1",
      "u": { "content": "U_1" }, "i": { "content": "I_1", "invert": false } },
    { "id": "op1", "type": "opamp", "at": [6, 1], "rot": 0, "flip": false, "label": "" },
    { "id": "n1", "type": "node", "at": [2, 0], "open": false },
    { "id": "a", "type": "terminal", "at": [-1, 0], "label": "a" }
  ],
  "wires": [ { "points": ["r1.out", [3, 0], [3, 2], "c1.in"] } ],
  "annotations": [
    { "type": "voltage", "from": "a", "to": "b", "content": "U_(a b)" },
    { "type": "current", "on": 0, "at": 0.5, "content": "I" },
    { "type": "text", "at": [1.5, 4], "content": "U_\"out\" = ..." }
  ]
}
```

- Grid units everywhere. Two-terminal parts use `from`/`to` (zap style, pins are the endpoints); multi-pin parts use `at` + `rot` + `flip` with pins from the part catalog.
- Wires are explicit polylines; endpoints may be pin refs (`"r1.out"`) so wires follow moved parts. Lossless round trip, no auto-routing to drift.
- `kind: "block"`: nodes `sum` (input signs), `gain`, `integrator`, `tf`, `function`, `io`, `branch`; edges `{from, to, label, sign, bends}`; `style.blocks: "laplace" | "time"`.
- `kind: "magnetic"`: core, windings, fluxes, sections and callouts placed on the core instead of the grid; see "Magnetic figures" below.
- `source` = provenance, so a generated figure can be regenerated.
- Writes are atomic (temp file + rename in the same folder) so tinymist and file-sync clients never see half-written JSON.

## Typst side

Course `template.typ` (generated by `lecfig course init`):

```typst
#import "@local/lecfig:0.1.0" as lecfig
#let fig(name, caption: none) = [#figure(
  lecfig.render(json("figs/" + name + ".json")), caption: caption,
) #label("fig:" + name)]
```

Package API: `render(data)` (dispatch on `kind`), `logic(expr)`, `sp(expr)`, `ode(expr)`, `loop(fwd, fb:)`, `magnetic(expr)`; the generator functions call the plugin and then `render`. Labels rendered with `eval(s, mode: "math")`. Editor mode (`render(data, editor: true)`) adds an invisible frame at known grid bounds so the editor can map pixels ↔ grid exactly.

## CLI (`lecfig`)

| Command | Effect | Prints |
| --- | --- | --- |
| `lecfig course init <dir>` | Copies `templates/course` there (template.typ, figs/, marker `lecfig.toml`) | — |
| `lecfig lecture [--for FILE]` | Creates today's `YYYY-MM-DD.typ` with template import | path |
| `lecfig new [name] [--kind block] [--flavor logic] [--from EXPR \| --preset NAME] [--for FILE]` | Creates `figs/<name or fig-NN>.json` (optionally generated or from a preset), spawns editor **detached** | `#fig("name")` |
| `lecfig edit <name\|path>` | Opens editor on existing figure | — |
| `lecfig mv <old> <new>` | Renames JSON and rewrites `#fig("old")` in the course's .typ files | — |

Course root = walk up from `--for FILE` (or cwd) to the `lecfig.toml` marker. Helix binding (documented in `docs/helix.md`, not installed automatically): `":insert-output lecfig new --for %{buffer_name}"` — verify Helix 25.07 expands `%{buffer_name}`.

## Editor

- eframe/egui window. Background thread compiles a tiny Typst doc (`render(data, editor: true)`, data passed as a virtual file) with the embedded compiler → typst-render pixmap → egui texture. Debounced (~50 ms); keeps last good image and shows Typst errors (e.g. bad label math) in a status bar. Overlay (cursor, pins, selection, wire in progress) drawn instantly from the data model.
- Autosave after every change (atomic). Undo/redo = snapshot stack of the figure struct.
- Draft keymap (lives in `~/.config/lecfig/keymap.toml`, defaults in code):

```
NORMAL   h j k l move · H J K L move 5 · v/click select · x add to selection
         m move (hjkl, Enter drops) · r rotate · F flip · d delete · y/p copy/paste
         c change label (INSERT, Typst math, Enter/Esc confirms) · u/U undo/redo
         w wire: hjkl routes, Enter = bend, Enter on pin = finish, Esc cancels
         a annotate: u voltage · i current · n node dot · t terminal · . text
                     · c callout (text with an arrow to what the cursor is on)
         .  repeat last placed part · q / :q done
space    part picker (which-key popup), first level depends on the figure's flavor:
         network     r R · c C · l L · z Z · u U-src · i I-src · g ground
         equivalent  r R · c C · l L · z Z · u/i src · U/I controlled src · g ground
         schematic   r R · c C · d diode · n npn · p pnp · m nmos · o op-amp · g ground
         logic       a AND · o OR · x XOR · n NOT · A NAND · O NOR · X XNOR · f flip-flop
         block       s sum · k gain · i integrator · t G(s) · f function · b branch · e in/out
         magnetic    w winding · f flux loop · y flux arrow on a leg (Ψ) · s cross-section
                     · g air gap
         every flavor: one more key opens submenus (s sources, e electronics, & logic,
         m meters, w switches, …) so every part is reachable everywhere;
         P opens the preset picker
```

## Milestones

Each ends with a check. M0–M3 is the thin end-to-end slice.

**M0 Setup & risk checks**
- `cargo install --locked typst-cli --version 0.15.1`; `rustup target add wasm32-unknown-unknown`; `cargo install just`.
- Compile samples: zap 0.6.0 and fletcher 0.5.8 on Typst 0.15.1; look at zap's IEC gates; check whether zap has controlled sources.
- Edit a JSON read by a `.typ` while the tinymist preview is open → preview refreshes?
- Check: one sample compiles; preview refresh confirmed (or workaround noted).

**M1 Data model + renderer (circuit subset)**
- `lecfig-core` serde structs + `docs/schema.md`; renderer for resistor, capacitor, inductor, vsource, isource, ground, node, terminal, wires, `u`/`i` labels.
- `templates/course` with the `fig` wrapper; `just install`.
- Tests: fixture JSONs in `tests/` compile with `typst compile`; serde round-trip test (load → save → identical).
- Check: hand-written `figs/rc.json` shows up in the preview via `#fig("rc")`.

**M2 CLI**
- `course init`, `lecture`, `new`, `edit`, `mv`; editor spawned detached; Helix doc.
- Check: from Helix, one key inserts `#fig("fig-01")` and an (empty) editor window opens.

**M3 Editor MVP**
- Embedded Typst render, grid cursor, modal keymap, flavor-aware picker for the M1 parts, wires, select/move/rotate/delete, label edit, auto-numbering, undo/redo, autosave, `q`.
- Pin table: `probe/pins.typ` dumps zap anchors via `typst query` → `crates/core/pins.json` (fallback: read zap source by hand).
- Check: draw an RC low-pass from scratch in the editor, close, reopen → identical file; preview matched the whole time.

**M4 Block diagrams + ODE generator** (flagship generator, see "ODE → block diagram" below)
- fletcher renderer (Laplace/time style), block parts in the editor.
- `lecfig-plugin` → `typst/lecfig/lecfig.wasm`; `#ode("m y'' + d y' + c y = F")`; `lecfig new --from-ode "…"` seeds the editor from the same core code.
- Unit tests: parser, rearrangement, layout snapshots (`cargo test`); fixture compiles.
- Check: `#ode` for a 1st-, 2nd- and 4th-order ODE renders correctly in the preview; generated figure opens in the editor and can be rearranged.

**M5 More generators**
- `#sp("R_1 || (R_3 + C_1)")`: leading letter → part type (R, L, C, Z, U/V, I, D …), identifier → label.
- `#logic("(a and b) or not c")`; `#loop(...)` / block algebra.
- `lecfig new --from "…"` for all of them.

**M6 Magnetic figures + presets** (see "Magnetic figures" below)
- `render-magnetic.typ` grown from the spike; layout ported to lecfig-core and exposed through the plugin, so renderer and editor share one geometry.
- `#magnetic("N_1 : N_2", …)` generator.
- Presets: built-in table in core, `~/.config/lecfig/presets.toml`, `lecfig new --preset NAME`, editor `space P`. First presets: `transformer`, `ideal-transformer`, `magnetic-circuit`.
- Editor magnetic flavor: cursor on the core, picker, sense/direction flip, callouts.
- Check: `#magnetic("N_1 : N_2", fluxes: "split", ideal: true)` matches the ideal-transformer slide; `lecfig new --preset ideal-transformer`, flip N_2's sense and move a callout in the editor, reopen → identical file.

**M7 Widen parts and annotations**
- Electronics (diode, BJT, MOSFET, op-amp), equivalent-circuit parts (controlled sources, Z — custom cetz symbols if zap lacks them), gates + flip-flops, voltage arrow between nodes, current on wire, free text.

**M8 Extras**
- Freeze to plain zap/fletcher code, more presets (voltage divider, RC low-pass, inverting amp, half-adder), Typst Universe check.

## ODE → block diagram

The textbook recipe is mechanical, so each rearranging step maps to one diagram element:

| Step on the equation | Block |
| --- | --- |
| Solve for the highest derivative | Summing junction collecting the right-hand side |
| Each derivative order | One integrator (`∫` or `1/s` per figure style): y'' → y' → y |
| Coefficient of a term | Gain block (label built from the source text, e.g. `d/m`; no CAS simplification) |
| Sign of a term | +/− at the summing junction |
| Divide by leading coefficient | One `1/a_n` gain after the sum (default) or merged into each gain (`normalize: "gains"`) |

Example: `m y'' + d y' + c y = F` → `y'' = 1/m (F − d y' − c y)` → F → (+) → [1/m] → ∫ → y' → ∫ → y, with feedback through [d] from y' and [c] from y.

- **v1 scope: linear ODE, any order** with constant coefficients, symbolic (`omega_0^2`, `2 D omega_0`) or numeric. Derivatives as primes (`y''`, `y'''`) or dots (`dot(y)`, `dot.double(y)`); identifiers are Typst math, so labels render as typed.
- Signal detection: the differentiated variable is the output; inputs are lone identifiers on the right (default `u`); overridable with `out:` / `in:`.
- `show-steps: true` renders the rearranged equation next to the diagram.
- Layout: input left, sum, optional 1/a_n gain, integrator chain left to right with taps; feedback gains stacked below their taps.
- Clear errors for out-of-scope input (input derivatives, nonlinear terms, highest derivative not isolable) that point to the backlog features.

**Backlog (tracked expansions, not v1):**
1. Nonlinear terms: `sin(y)`, `y y'`, `y'^2` → function and multiplier blocks in feedback paths.
2. Input derivatives: `b_1 u' + b_0 u` → controllable / observable canonical form (Regelungs-/Beobachtungsnormalform), selectable per figure.
3. Coupled systems / state space: several equations or `x' = A x + B u` → several chains with cross connections.
4. Laplace-only variants: single `G(s)` block; series/parallel decomposition for numeric coefficients.

## Magnetic figures

Transformers and magnetic circuits come up regularly in the lectures: an iron core in section, windings around its legs, flux paths inside the core, u/i arrows at the terminals, and notes such as `R_1 = 0`. The ideal-transformer slide is the reference; `tests/magnetic/transformer.typ` reproduces it from data with `typst/lecfig/render-magnetic.typ`, along with two variants (opposite winding sense, single coil with an air gap).

**Positions live on the core, not on the grid.** A point is `(leg, pos, lane)`: `leg` is `left`, `right`, `top` or `bottom` (the last two are the yokes), `pos` runs 0..1 along the window edge, and `lane` goes from −1 (window edge) through 0 (centre line) to 1 (outer edge). Resizing the core moves everything with it, and flux paths stay inside the core.

```json
{
  "version": 1,
  "kind": "magnetic",
  "source": { "generator": "magnetic", "expr": "N_1 : N_2", "fluxes": "split", "ideal": true },
  "style": { "colors": "lecture" },
  "core": { "shape": "ring", "size": [6, 3.7], "leg": 0.9, "yoke": 0.6, "label": "mu", "gaps": [] },
  "windings": [
    { "id": "w1", "leg": "left", "sense": "cw", "turns": 3, "label": "N_1",
      "port": { "i": "i_1", "u": "u_1" } },
    { "id": "w2", "leg": "right", "sense": "cw", "turns": 3, "label": "N_2",
      "port": { "i": "i_2", "i-dir": "out", "u": "u_2" } }
  ],
  "fluxes": [
    { "id": "f1", "path": "loop", "lane": 0.45, "dir": "cw", "arrows": [{ "leg": "top", "pos": 0.75 }] },
    { "id": "psi1", "path": "left", "lane": 0.1, "dir": "cw", "label": "Psi_1" }
  ],
  "sections": [{ "id": "s1", "leg": "right", "pos": 0.96 }],
  "annotations": [
    { "type": "callout", "tone": "magnetic", "content": "Phi_h = Phi_21 - Phi_12",
      "to": "s1.north", "rel": [0.4, 1.2] },
    { "type": "callout", "content": "R_1 = 0", "to": "w1.lead2", "rel": [-0.3, -0.9] }
  ]
}
```

- `sense` is physical, not graphical: `cw` means current into terminal 1 drives a clockwise flux. The renderer derives the drawing from it with the right-hand rule: which edge the front strokes start from, and whether lead 1 crosses the front of the leg. Flipping `sense` therefore always gives a correctly drawn winding.
- `turns` is how many turns are drawn; the label carries the real number (`N_1`).
- A flux `path` is `loop` (closed path around the core at one lane, with arrowheads where listed) or a leg name (one straight arrow, e.g. a linkage Ψ).
- `callout` (text plus pointer arrow) is the one annotation for all notes: flux names, `R_1 = 0`, `P_"Fe" = 0`. Targets are named anchors (`w1.t1`, `w1.lead2`, `s1.north`) or core points (`{ "leg": "bottom", "pos": 0.85, "lane": 1 }`, or `{ "flux": "f1", "leg": "top", "pos": 0.6 }` to hit a flux line). `rel` places the text relative to its target, so it moves along. `tone` picks the colour. Callouts are useful in circuits too, so they belong to every kind.
- `style.colors`: `lecture` (electrical red, magnetic green, as on the slides) or `mono`.

**Generator** `#magnetic(expr, ..options)`, in the Rust core like the others:

| Input | Result |
| --- | --- |
| `N` | one winding on the left leg, main flux Φ, port u, i |
| `N_1 : N_2` | windings on the left and right legs with the same sense (current into both top terminals drives the same flux) |
| `N_1 : -N_2` | `-` reverses that winding's sense |
| `fluxes: "main"` (default) · `"split"` · `"none"` | one loop Φ_h · loops Φ_11 = Φ_21 and Φ_12 = Φ_22 with linkages Ψ_1, Ψ_2 and the Φ_h cross-section, as on the slide · no fluxes |
| `ideal: true` | callouts `R_k = 0` and `P_"Fe" = 0` |
| `gap: "delta"` | air gap in the first leg without a winding, with its label |
| `arrows: "consumer-generator"` (default) · `"consumer"` | consumer arrows on the primary and generator arrows on the secondary · consumer arrows on both |

Port and flux labels follow the winding label's subscript: `N_1` → `u_1`, `i_1`, `Psi_1`; `N` → `u`, `i`, `Psi`. Identifiers are Typst math, as everywhere else.

**Presets.** A preset is a named generator call: `transformer` = `magnetic("N_1 : N_2")`, `ideal-transformer` = `magnetic("N_1 : N_2", fluxes: "split", ideal: true)`, `magnetic-circuit` = `magnetic("N", gap: "delta")`. Built-in ones live in lecfig-core; your own go in `~/.config/lecfig/presets.toml`. Three ways in:

- **Typst:** the generator call itself, no editor needed.
- **CLI:** `lecfig new --preset ideal-transformer` writes the figure and opens the editor.
- **Editor:** `space P` opens the preset picker in every flavor and fills an empty figure. Inserting a preset into a non-empty figure (e.g. a voltage divider into a larger circuit) is backlog.

The figure keeps `source`, so the editor can change the expression (`N_1 : N_2` → `N_1 : -N_2`) and regenerate. Regenerating drops manual edits, so the editor asks first.

**Editor (magnetic flavor).** The cursor walks the core: `hjkl` moves along a leg or across lanes and turns the corners onto the next leg. Picker: `w` winding, `f` flux loop, `y` flux arrow on a leg, `s` cross-section, `g` air gap; callouts via `a c`. On a selection: `F` reverses a winding's sense or a flux's direction, `+`/`-` changes the drawn turns, `m` moves (windings and arrowheads along their leg, loops across lanes, callout text freely), `c` edits the label. Core size and thicknesses by dragging an edge with the mouse.

**One geometry for renderer and editor.** Winding strokes, lane lines and terminal positions are computed, not stored. The renderer needs them to draw; the editor needs them for hit-testing and its overlay. So the layout moves into lecfig-core (`magnetic::layout`, data in, absolute points out), the plugin exposes it, and `render-magnetic.typ` only draws what it receives. The Typst spike is the reference implementation and serves as the test oracle for the port.

**Backlog (not M6):**
1. Leakage fluxes (Φ_σ1, Φ_σ2) closing through air around one winding.
2. Shell core (three legs, windings on the centre leg), toroid.
3. Several windings on one leg (stacked or concentric).
4. Derive the magnetic equivalent circuit (sources Θ = N i, reluctances R_m) as a `kind: "circuit"` figure.

## Verification (end-to-end, after M3)

1. `just install && just test` (cargo tests + Typst fixture compiles).
2. `lecfig course init ~/notes/test-course`, open a lecture file in Helix with the tinymist preview running.
3. Helix key → `#fig("fig-01")` inserted, editor opens; draw R, C, source, ground, wires, labels; watch the browser preview update while drawing.
4. `q`, reopen with `lecfig edit fig-01` → nothing lost (`git diff` on the JSON shows no change).
5. `#ode("m y'' + d y' + c y = F", show-steps: true)` renders the rearranged equation and the integrator chain (after M4).
6. `#sp("R_1 || (R_3 + C_1)")` in the notes renders (after M5).
7. `#magnetic("N_1 : N_2", fluxes: "split", ideal: true)` renders the ideal-transformer slide (after M6).
