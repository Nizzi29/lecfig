// Renders circuit figures (kind "circuit") with zap. The ONLY file that calls zap.
#import "@preview/zap:0.6.0"
#import "common.typ": callout, cetz, current-arrow, lbl, vec, voltage-arrow

// Parts drawn between two points (pins "in" and "out").
#let two-terminal = (
  resistor: zap.resistor,
  impedance: zap.resistor,
  capacitor: zap.capacitor,
  pcapacitor: zap.pcapacitor,
  inductor: zap.inductor,
  vsource: zap.vsource,
  isource: zap.isource,
  acvsource: zap.acvsource,
  dvsource: zap.dvsource,
  disource: zap.disource,
  battery: zap.battery,
  diode: zap.diode,
  led: zap.led,
  zener: zap.zener,
  switch: zap.switch,
  fuse: zap.fuse,
  lamp: zap.lamp,
  voltmeter: zap.voltmeter,
  ammeter: zap.ammeter,
  potentiometer: zap.potentiometer,
)

// Parts placed at one point, rotated by `rot`.
#let point-parts = (
  ground: zap.ground,
  earth: zap.earth,
  vcc: zap.vcc,
  vee: zap.vee,
  opamp: zap.opamp,
  npn: zap.npn,
  pnp: zap.pnp,
  nmos: zap.nmos,
  pmos: zap.pmos,
  land: zap.land,
  lor: zap.lor,
  lxor: zap.lxor,
  lnand: zap.lnand,
  lnor: zap.lnor,
  lxnor: zap.lxnor,
  lnot: zap.lnot,
  dflipflop: zap.dflipflop,
  jkflipflop: zap.jkflipflop,
  srlatch: zap.srlatch,
)

#let gates = ("land", "lor", "lxor", "lnand", "lnor", "lxnor")

// A u/i decoration: Typst math, or (content, invert, anchor).
#let deco(d) = {
  if type(d) == str { return lbl(d) }
  let out = (content: lbl(d.content))
  if d.at("invert", default: false) { out.invert = true }
  if "anchor" in d { out.anchor = d.anchor }
  out
}

// Offsets of n pins along a side of height h (mirrors parts::side_pins in Rust).
#let side-pins(n, h) = {
  let s = calc.max(1, calc.floor(h / calc.max(n, 1)))
  if calc.rem((calc.max(n, 1) - 1) * s, 2) == 1 and s > 1 { s -= 1 }
  range(n).map(k => ((n - 1) / 2 - k) * s)
}

// Exact quarter turns, flip first: the same order as parts::pins in Rust.
#let turn(p, rot, flip) = {
  let (x, y) = p
  if flip { y = -y }
  let r = calc.rem(calc.rem(rot, 360) + 360, 360)
  if r == 90 { (-y, x) } else if r == 180 { (-x, -y) } else if r == 270 { (y, -x) } else { (x, y) }
}

// A labeled box with pins on its left (in1 …) and right (out1 …) sides.
#let draw-block(e) = {
  import cetz.draw: *
  let (w, h) = e.at("size", default: (3, 4))
  let (nin, nout) = (e.at("inputs", default: 2), e.at("outputs", default: 2))
  let (rot, flip) = (e.at("rot", default: 0), e.at("flip", default: false))
  let at = e.at("at")
  let place(p) = vec.add(at, turn(p, rot, flip))
  group(name: e.id, {
    let (a, b) = (place((-w / 2, -h / 2)), place((w / 2, h / 2)))
    rect(a, b, stroke: 0.8pt)
    if e.at("label", default: "") != "" {
      content(at, math.equation(block: true, eval(e.label, mode: "math")))
    }
    for (k, y) in side-pins(nin, h).enumerate() { anchor("in" + str(k + 1), place((-w / 2, y))) }
    for (k, y) in side-pins(nout, h).enumerate() { anchor("out" + str(k + 1), place((w / 2, y))) }
  })
}

// Two coils facing each other, with an iron core (two lines) unless core: false.
// Pins p1/p2 (primary, left) and s1/s2 (secondary, right) sit on the grid.
#let draw-transformer(e) = {
  import cetz.draw: *
  let stroke = 0.6pt
  group(name: e.id, {
    set-origin(e.at("at"))
    rotate(e.at("rot", default: 0) * 1deg)
    if e.at("flip", default: false) { scale(y: -1) }
    let (x, r, n) = (0.45, 0.25, 4)
    for (side, bulge) in ((-1, -1), (1, 1)) {
      let cx = side * x
      // Leads from the pins to the coil ends.
      line((side, 1), (cx, 1), stroke: stroke)
      line((side, -1), (cx, -1), stroke: stroke)
      for k in range(n) {
        let y = 1 - 2 * r * k
        // Left coil bulges right (towards the core), right coil bulges left.
        if side == -1 {
          arc((cx, y), start: 90deg, stop: -90deg, radius: r, stroke: stroke)
        } else {
          arc((cx, y), start: 90deg, stop: 270deg, radius: r, stroke: stroke)
        }
      }
    }
    if e.at("core", default: true) {
      for cx in (-0.08, 0.08) { line((cx, 1.1), (cx, -1.1), stroke: 0.9pt) }
    }
    anchor("p1", (-1, 1))
    anchor("p2", (-1, -1))
    anchor("s1", (1, 1))
    anchor("s2", (1, -1))
    if e.at("label", default: "") != "" { content((0, 1.25), lbl(e.label), anchor: "south") }
  })
}

#let draw-element(e) = {
  let kind = e.type
  let args = (:)
  if e.at("label", default: "") != "" { args.label = lbl(e.label) }
  if "u" in e { args.u = deco(e.u) }
  if "i" in e { args.i = deco(e.i) }
  if kind in two-terminal {
    if kind == "impedance" { args.variant = "iec" }
    (two-terminal.at(kind))(e.id, e.from, e.to, ..args)
  } else if kind == "node" {
    zap.node(e.id, e.at("at"), fill: not e.at("open", default: false), label: args.at("label", default: none))
  } else if kind == "block" {
    draw-block(e)
  } else if kind == "transformer" {
    draw-transformer(e)
  } else if kind == "terminal" {
    zap.node(e.id, e.at("at"), fill: false, label: args.at("label", default: none))
  } else if kind in point-parts {
    args.angle = e.at("rot", default: 0) * 1deg
    if kind == "opamp" and e.at("flip", default: false) {
      args.invert = true
    } else if e.at("flip", default: false) {
      args.scale = (x: 1, y: -1)
    }
    if kind in gates and "inputs" in e { args.inputs = e.inputs }
    (point-parts.at(kind))(e.id, e.at("at"), ..args)
  } else {
    panic("lecfig: unknown part type '" + kind + "'")
  }
}

#let render-circuit(data, editor: false) = {
  let elements = data.at("elements", default: ())
  // Single-point parts are referenced by their bare id; zap knows them by position.
  let points = (:)
  for e in elements {
    if "at" in e { points.insert(e.id, e.at("at")) }
  }
  let resolve(p) = {
    if type(p) == str and not p.contains(".") { points.at(p, default: p) } else { p }
  }

  zap.circuit({
    import cetz.draw: *
    for e in elements { draw-element(e) }
    for w in data.at("wires", default: ()) {
      let pts = w.points.map(resolve)
      zap.wire(..pts)
      if "i" in w {
        get-ctx(ctx => {
          let (_, ..ps) = cetz.coordinate.resolve(ctx, ..pts)
          let d = w.i
          let (body, inv) = if type(d) == str { (d, false) } else { (d.content, d.at("invert", default: false)) }
          current-arrow(ps.map(p => (p.at(0), p.at(1))), lbl(body), invert: inv)
        })
      }
    }
    for (n, a) in data.at("annotations", default: ()).enumerate() {
      if a.type == "text" {
        content(a.at("at"), lbl(a.content))
      } else if a.type == "callout" {
        callout(resolve(a.to), a.rel, lbl(a.content), "lecfig-callout-" + str(n))
      } else if a.type == "voltage" {
        get-ctx(ctx => {
          let (_, p, q) = cetz.coordinate.resolve(ctx, resolve(a.from), resolve(a.to))
          let (p, q) = ((p.at(0), p.at(1)), (q.at(0), q.at(1)))
          if a.at("invert", default: false) { (p, q) = (q, p) }
          voltage-arrow(p, q, lbl(a.content), bend: a.at("bend", default: 0.5))
        })
      }
    }
    // Editor mode: two invisible markers let the editor map grid units to pixels.
    if editor {
      content((0, 0), [#metadata((0, 0))<lecfig-grid>])
      content((1, 1), [#metadata((1, 1))<lecfig-grid>])
    }
  })
}
