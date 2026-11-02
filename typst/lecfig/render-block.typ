// Renders block diagrams (kind "block") with fletcher. The ONLY file that calls fletcher.
#import "@preview/fletcher:0.5.8" as fletcher: diagram, edge, node
#import "common.typ": lbl, txt

// Content of a node; integrators and empty transfer functions get their symbol.
#let node-body(n, laplace) = {
  let s = n.at("label", default: "")
  if s != "" { return lbl(s) }
  if n.type == "integrator" { if laplace { $1/s$ } else { $integral$ } } else { [] }
}

#let render-block(data, editor: false) = {
  // From schema version 2 on, input/output labels are markup (text, $math$).
  let io-markup = data.at("version", default: 1) >= 2
  let laplace = data.at("style", default: (:)).at("blocks", default: "laplace") == "laplace"
  let nodes = data.at("nodes", default: ())
  let kinds = (:)
  for n in nodes { kinds.insert(n.id, n.type) }
  let cell(p) = (p.at(0), p.at(1))

  // Editor mode: a marker at each node centre tells the editor where the node landed.
  let mark(n, body) = if editor { [#body#place(center + horizon, [#metadata(n.id)<lecfig-node>])] } else { body }

  let items = ()
  for n in nodes {
    let pos = cell(n.at("at"))
    let name = label(n.id)
    let t = n.type
    if t == "sum" {
      items.push(node(pos, mark(n, []), shape: circle, radius: 0.45em, name: name))
    } else if t == "branch" {
      items.push(node(pos, mark(n, []), shape: circle, radius: 1.6pt, fill: black, stroke: none, name: name))
    } else if t == "box" {
      // Generic block: the label is Typst markup (text, $math$).
      items.push(node(pos, mark(n, txt(n.at("label", default: ""))), shape: rect, name: name))
    } else if t == "io" {
      // Multi-line labels (lists) read best left-aligned.
      let body = if io-markup { align(left, txt(n.at("label", default: ""))) } else { node-body(n, laplace) }
      items.push(node(pos, mark(n, body), stroke: none, name: name))
    } else {
      // gain, integrator, tf, function
      items.push(node(pos, mark(n, node-body(n, laplace)), shape: rect, name: name))
    }
  }
  // Editor mode: invisible markers on every cell of the used range, so the editor knows
  // where rows and columns without nodes (e.g. feedback lines) really are.
  if editor and nodes.len() > 0 {
    let cells = nodes.map(n => cell(n.at("at")))
    for e in data.at("edges", default: ()) { cells += e.at("bends", default: ()).map(cell) }
    let (xs, ys) = (cells.map(c => c.at(0)), cells.map(c => c.at(1)))
    for x in range(calc.min(..xs), calc.max(..xs) + 1) {
      for y in range(calc.min(..ys), calc.max(..ys) + 1) {
        let id = "cell:" + str(x) + ":" + str(y)
        items.push(node((x, y), [#metadata(id)<lecfig-node>], inset: 0pt, stroke: none, snap: false))
      }
    }
  }
  for e in data.at("edges", default: ()) {
    let verts = (label(e.from), ..e.at("bends", default: ()).map(cell), label(e.to))
    // A line into a branch point ends without an arrow head; the signal continues.
    let marks = if kinds.at(e.to, default: "") == "branch" { "-" } else { "-|>" }
    let args = (:)
    if e.at("label", default: "") != "" { args.label = lbl(e.label) }
    items.push(edge(..verts, marks, ..args))
    // The sign sits next to the arrow head, as an invisible twin of the edge.
    if e.at("sign", default: "") != "" {
      // On the last segment, just before the arrow head, beside the line (fletcher would
      // otherwise put it on the longest segment of a bent line).
      items.push(edge(
        ..verts,
        "-",
        stroke: none,
        label: text(0.9em, eval(e.sign, mode: "math")),
        label-pos: (verts.len() - 2, 100% - 0.9em),
        label-side: left,
        label-sep: 0.2em,
      ))
    }
  }

  diagram(
    node-stroke: 0.6pt,
    edge-stroke: 0.6pt,
    spacing: (2.3em, 1.5em),
    node-inset: 0.55em,
    ..items,
  )
}
