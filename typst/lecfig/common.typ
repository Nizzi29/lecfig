// Helpers shared by the renderers: labels, colours, callouts.
#import "@preview/cetz:0.5.2"

#let vec = cetz.vector

// Electrical quantities red, magnetic ones green, as on the lecture slides.
#let palettes = (
  lecture: (core: black, electric: rgb("#d7191c"), magnetic: rgb("#1a9641"), note: black),
  mono: (core: black, electric: black, magnetic: black, note: black),
)

// Labels are stored as Typst math source. Empty strings render as nothing.
#let lbl(s) = if s == none or s == "" { none } else { eval(s, mode: "math") }

// The compass anchor of a label that faces direction `d`.
#let facing(d) = {
  let names = ("east", "north-east", "north", "north-west", "west", "south-west", "south", "south-east")
  names.at(calc.rem(int(calc.round(calc.atan2(..d) / 45deg)) + 8, 8))
}

// Text placed `rel` away from `to` (a cetz coordinate), with an arrow to it.
#let callout(to, rel, body, name, color: black, arrow: black) = {
  import cetz.draw: *
  content((rel: rel, to: to), text(fill: color, body), name: name, padding: 0.06)
  line(
    name + "." + facing(vec.neg(rel)),
    to,
    stroke: (paint: arrow, thickness: 0.5pt),
    mark: (end: ">", fill: arrow, scale: 0.9),
  )
}

// A curved voltage arrow from `a` to `b` (numeric points), bulging by `bend` to the
// left of a→b (negative: to the right), with its label beyond the bulge.
#let voltage-arrow(a, b, body, bend: 0.5, color: black) = {
  import cetz.draw: *
  let along = vec.norm(vec.sub(b, a))
  let left = (-along.at(1), along.at(0))
  let off = vec.scale(left, bend)
  let a2 = vec.add(vec.add(a, vec.scale(along, 0.15)), vec.scale(off, 0.25))
  let b2 = vec.add(vec.sub(b, vec.scale(along, 0.15)), vec.scale(off, 0.25))
  bezier(
    a2,
    b2,
    vec.add(a2, off),
    vec.add(b2, off),
    stroke: (paint: color, thickness: 0.8pt),
    mark: (end: ">", fill: color, scale: 1.1),
  )
  let mid = vec.add(vec.lerp(a, b, 0.5), vec.scale(off, 1.0))
  content(mid, text(fill: color, body), anchor: facing(vec.neg(off)), padding: 0.08)
}

// A current arrow head on the longest segment of a polyline (numeric points), with
// its label beside it. zap 0.6.0's own `wire(i: …)` panics, hence this.
#let current-arrow(points, body, invert: false, color: black) = {
  import cetz.draw: *
  let best = 0
  for k in range(points.len() - 1) {
    if vec.dist(points.at(k), points.at(k + 1)) > vec.dist(points.at(best), points.at(best + 1)) { best = k }
  }
  let (a, b) = (points.at(best), points.at(best + 1))
  if invert { (a, b) = (b, a) }
  let mid = vec.lerp(a, b, 0.5)
  let along = vec.norm(vec.sub(b, a))
  mark(mid, vec.add(mid, along), symbol: ">", anchor: "center", fill: color, stroke: color, scale: 1.1)
  let side = (-along.at(1), along.at(0))
  // Labels go above horizontal wires and left of vertical ones.
  if side.at(1) < -0.5 or side.at(0) > 0.5 { side = vec.neg(side) }
  content(vec.add(mid, vec.scale(side, 0.12)), text(fill: color, body), anchor: facing(vec.neg(side)))
}
