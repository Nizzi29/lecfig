// Renders magnetic figures (kind "magnetic"): an iron core drawn in section,
// windings wrapped around its legs, flux paths inside it, ports and callouts.
// zap has no core or winding symbols, so this file draws with cetz directly,
// pinned to the cetz version zap 0.6.0 uses.
#import "common.typ": callout, cetz, facing, lbl, palettes, vec

// Coordinates are cetz units (1 = 1 cm), y up, the core centred on the origin.
// A point on the core is (leg, pos, lane):
//   leg   "left" | "right" | "top" | "bottom" (top and bottom are the yokes)
//   pos   0..1 along the window edge: bottom → top on legs, left → right on yokes
//   lane  -1 window edge · 0 centre line · 1 outer edge
// Per leg: centre `c`, axis `e` (direction of growing pos), outward normal `n`,
// half thickness, window length `len`, outer length `full`, and `cw`, the sign
// a clockwise flux has along `e`.
#let core-legs(core) = {
  let (w, h) = core.size
  let (a, b) = (core.leg, core.yoke)
  (
    left: (c: (-w / 2 + a / 2, 0), e: (0, 1), n: (-1, 0), half: a / 2, len: h - 2 * b, full: h, cw: 1),
    right: (c: (w / 2 - a / 2, 0), e: (0, 1), n: (1, 0), half: a / 2, len: h - 2 * b, full: h, cw: -1),
    top: (c: (0, h / 2 - b / 2), e: (1, 0), n: (0, 1), half: b / 2, len: w - 2 * a, full: w, cw: 1),
    bottom: (c: (0, -h / 2 + b / 2), e: (1, 0), n: (0, -1), half: b / 2, len: w - 2 * a, full: w, cw: -1),
  )
}

#let leg-point(leg, pos, lane) = vec.add(
  vec.add(leg.c, vec.scale(leg.e, (pos - 0.5) * leg.len)),
  vec.scale(leg.n, lane * leg.half),
)

// A target is a cetz anchor name ("w1.lead2", "s1.north") or a core point,
// whose lane may come from a flux: (flux: "f1", leg: "top", pos: 0.6).
#let resolve(legs, fluxes, target) = {
  if type(target) in (str, array) { return target }
  let lane = if "flux" in target {
    fluxes.find(f => f.id == target.flux).lane
  } else {
    target.at("lane", default: 0)
  }
  leg-point(legs.at(target.leg), target.pos, lane)
}

// Outline of the core, broken by air gaps: (leg, pos, width in cm).
#let draw-core(legs, core, col) = {
  import cetz.draw: *
  let stroke = (paint: col, thickness: 1pt, cap: "square")
  let gaps = core.at("gaps", default: ())
  for (name, leg) in legs {
    let mine = gaps.filter(g => g.leg == name).sorted(key: g => g.pos)
    for lane in (-1, 1) {
      // Window edges span the window, outer edges the whole core.
      let ext = if lane == 1 { leg.full / leg.len / 2 } else { 0.5 }
      let cuts = (0.5 - ext,)
      for g in mine {
        let half = g.width / leg.len / 2
        cuts += (g.pos - half, g.pos + half)
      }
      cuts.push(0.5 + ext)
      for i in range(0, cuts.len(), step: 2) {
        line(leg-point(leg, cuts.at(i), lane), leg-point(leg, cuts.at(i + 1), lane), stroke: stroke)
      }
    }
    for g in mine {
      let half = g.width / leg.len / 2
      for pos in (g.pos - half, g.pos + half) {
        line(leg-point(leg, pos, -1), leg-point(leg, pos, 1), stroke: stroke)
      }
    }
  }
}

// A flux is either a closed loop around the core (path "loop") at one lane,
// or a straight arrow along one leg (path = the leg's name), e.g. a linkage Ψ.
#let draw-flux(legs, f, col) = {
  import cetz.draw: *
  let s = if f.at("dir", default: "cw") == "cw" { 1 } else { -1 }
  let stroke = (paint: col, thickness: 1.2pt)
  if f.path == "loop" {
    let lo = (leg-point(legs.left, 0, f.lane).at(0), leg-point(legs.bottom, 0, f.lane).at(1))
    let hi = (leg-point(legs.right, 0, f.lane).at(0), leg-point(legs.top, 0, f.lane).at(1))
    rect(lo, hi, stroke: stroke)
    for a in f.at("arrows", default: ()) {
      let leg = legs.at(a.leg)
      let p = leg-point(leg, a.pos, f.lane)
      mark(p, vec.add(p, vec.scale(leg.e, s * leg.cw)), symbol: ">", anchor: "center", fill: col, stroke: col, scale: 1.4)
    }
  } else {
    let leg = legs.at(f.path)
    let (p0, p1) = f.at("span", default: (0.15, 0.85))
    if (p1 - p0) * s * leg.cw < 0 { (p0, p1) = (p1, p0) }
    line(leg-point(leg, p0, f.lane), leg-point(leg, p1, f.lane), stroke: stroke, mark: (end: ">", fill: col, scale: 1.4))
    if "label" in f {
      let side = if f.at("label-side", default: "in") == "in" { -1 } else { 1 }
      let d = vec.scale(leg.n, side)
      content(vec.add(leg-point(leg, 0.5, side), vec.scale(d, 0.4)), text(fill: col, lbl(f.label)), anchor: facing(vec.neg(d)))
    }
  }
}

// Where a winding's wire runs. The wire is drawn as seen from the front: front
// strokes cross the leg from the a-lane edge to the b-lane edge, back strokes
// are hidden, and small bumps show the wire turning around an edge.
#let winding-layout(legs, w) = {
  let leg = legs.at(w.leg)
  let s = if w.at("sense", default: "cw") == "cw" { 1 } else { -1 }
  // Right-hand rule: the flux current into terminal 1 drives along the leg,
  // and the direction that current takes across the front of the leg.
  let f = vec.scale(leg.e, s * leg.cw)
  let front = (f.at(1), -f.at(0))
  let b-lane = vec.dot(front, leg.n)
  let a-lane = -b-lane
  let t-lane = if w.at("terminals", default: "out") == "out" { 1 } else { -1 }
  let (from, to) = w.at("span", default: if leg.e == (0, 1) { (0.85, 0.15) } else { (0.15, 0.85) })
  let dir = if to > from { 1 } else { -1 }
  let slant = dir * 0.22 / leg.len
  let bump = dir * 0.16 / leg.len
  let turns = w.at("turns", default: 3)
  // If front strokes start on the terminal side, lead 1 is the first of them.
  let lead-first = a-lane == t-lane
  let t0 = if lead-first { from } else { from + slant / 2 + 2 * bump }
  let tn = if b-lane == t-lane { to - slant / 2 } else { to - slant / 2 - 2 * bump }
  // A single turn sits at whichever end is fixed: lead 1 crossing, or lead 2 leaving.
  let centres = if turns == 1 {
    (if lead-first { t0 } else { tn },)
  } else {
    range(turns).map(k => t0 + k * (tn - t0) / (turns - 1))
  }
  let strokes = ()
  let bumps = ()
  for (k, t) in centres.enumerate() {
    let (ta, tb) = if k == 0 and lead-first { (t, t) } else { (t - slant / 2, t + slant / 2) }
    if not (k == 0 and lead-first) {
      bumps.push((a-lane, ta - bump, ta))
      strokes.push((ta, tb))
    }
    // The last stroke leaves straight through lead 2 if it ends on the terminal side.
    if not (k == turns - 1 and b-lane == t-lane) { bumps.push((b-lane, tb, tb + bump)) }
  }
  let out = vec.scale(leg.n, t-lane)
  let lead = w.at("lead", default: 1.1)
  let edge1 = leg-point(leg, from, t-lane)
  let edge2 = leg-point(leg, to, t-lane)
  (
    leg: leg,
    dir: dir,
    out: out,
    a-lane: a-lane,
    b-lane: b-lane,
    strokes: strokes,
    bumps: bumps,
    edge1: edge1,
    edge2: edge2,
    t1: vec.add(edge1, vec.scale(out, lead)),
    t2: vec.add(edge2, vec.scale(out, lead)),
    lead1-end: if lead-first { leg-point(leg, from, b-lane) } else { edge1 },
  )
}

// Current arrow on lead 1 and a curved voltage arrow between the terminals.
#let draw-port(l, p, pal) = {
  import cetz.draw: *
  let col = pal.electric
  if "i" in p {
    let at = vec.lerp(l.t1, l.edge1, 0.4)
    let d = if p.at("i-dir", default: "in") == "in" { vec.neg(l.out) } else { l.out }
    mark(at, vec.add(at, d), symbol: ">", anchor: "center", fill: col, stroke: col, scale: 1.2)
    let away = vec.scale(l.leg.e, -l.dir)
    content(vec.add(at, vec.scale(away, 0.12)), text(fill: col, lbl(p.i)), anchor: facing(vec.neg(away)))
  }
  if "u" in p {
    let (a, b) = if p.at("u-dir", default: "1-2") == "1-2" { (l.t1, l.t2) } else { (l.t2, l.t1) }
    let along = vec.norm(vec.sub(b, a))
    let a2 = vec.add(vec.add(a, vec.scale(along, 0.18)), vec.scale(l.out, 0.12))
    let b2 = vec.add(vec.sub(b, vec.scale(along, 0.18)), vec.scale(l.out, 0.12))
    let bulge = vec.scale(l.out, 0.5)
    bezier(a2, b2, vec.add(a2, bulge), vec.add(b2, bulge), stroke: (paint: col, thickness: 0.8pt), mark: (end: ">", fill: col, scale: 1.2))
    content(vec.add(vec.lerp(a, b, 0.5), vec.scale(l.out, 0.6)), text(fill: col, lbl(p.u)), anchor: facing(vec.neg(l.out)))
  }
}

#let draw-winding(legs, w, pal) = {
  import cetz.draw: *
  let l = winding-layout(legs, w)
  let leg = l.leg
  let wire = (paint: pal.core, thickness: 0.6pt)
  group(name: w.id, {
    for (ta, tb) in l.strokes {
      line(leg-point(leg, ta, l.a-lane), leg-point(leg, tb, l.b-lane), stroke: wire)
    }
    for (lane, ts, te) in l.bumps {
      let (p, q) = (leg-point(leg, ts, lane), leg-point(leg, te, lane))
      let push = vec.scale(leg.n, lane * 0.12)
      bezier(p, q, vec.add(p, push), vec.add(q, push), stroke: wire)
    }
    line(l.t1, l.lead1-end, stroke: wire)
    line(l.edge2, l.t2, stroke: wire)
    for t in (l.t1, l.t2) { circle(t, radius: 0.07, fill: white, stroke: wire) }
    anchor("t1", l.t1)
    anchor("t2", l.t2)
    anchor("lead1", vec.lerp(l.t1, l.edge1, 0.5))
    anchor("lead2", vec.lerp(l.t2, l.edge2, 0.5))
    if "label" in w {
      content(vec.add(vec.lerp(l.edge1, l.edge2, 0.5), vec.scale(l.out, 0.3)), lbl(w.label), anchor: facing(vec.neg(l.out)))
    }
    if "port" in w { draw-port(l, w.port, pal) }
  })
}

// A cross-section marker: an ellipse across a leg, named so callouts can point at it.
#let draw-section(legs, s, col) = {
  import cetz.draw: *
  let leg = legs.at(s.leg)
  let across = leg.half * s.at("size", default: 0.8)
  let r = if leg.e == (0, 1) { (across, 0.1) } else { (0.1, across) }
  circle(leg-point(leg, s.pos, 0), radius: r, stroke: (paint: col, thickness: 0.6pt), name: s.id)
}

#let draw-callout(legs, fluxes, a, name, pal) = {
  let col = pal.at(a.at("tone", default: "note"))
  callout(resolve(legs, fluxes, a.to), a.rel, lbl(a.content), name, color: col, arrow: pal.note)
}

#let render-magnetic(data, editor: false) = {
  let pal = palettes.at(data.at("style", default: (:)).at("colors", default: "lecture"))
  let legs = core-legs(data.core)
  let fluxes = data.at("fluxes", default: ())
  cetz.canvas({
    import cetz.draw: *
    draw-core(legs, data.core, pal.core)
    for f in fluxes { draw-flux(legs, f, pal.magnetic) }
    for w in data.at("windings", default: ()) { draw-winding(legs, w, pal) }
    for s in data.at("sections", default: ()) { draw-section(legs, s, pal.note) }
    for (i, a) in data.at("annotations", default: ()).enumerate() {
      if a.type == "callout" { draw-callout(legs, fluxes, a, "lecfig-callout-" + str(i), pal) }
    }
    // Last, so it sits on top of the flux lines it usually overlaps.
    if "label" in data.core {
      let at = resolve(legs, fluxes, data.core.at("label-at", default: (leg: "top", pos: 0.3)))
      content(at, lbl(data.core.label), frame: "rect", fill: white, stroke: none, padding: 0.03)
    }
    if editor {
      content((0, 0), [#metadata((0, 0))<lecfig-grid>])
      content((1, 1), [#metadata((1, 1))<lecfig-grid>])
    }
  })
}
