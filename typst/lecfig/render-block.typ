// Renders block diagrams (kind "block") with fletcher. The ONLY file that calls fletcher.
#import "@preview/fletcher:0.5.8" as fletcher: diagram, edge, node
#import "common.typ": lbl

// Content of a node; integrators and empty transfer functions get their symbol.
#let node-body(n, laplace) = {
  let s = n.at("label", default: "")
  if s != "" { return lbl(s) }
  if n.type == "integrator" { if laplace { $1/s$ } else { $integral$ } } else { [] }
}

#let render-block(data, editor: false) = {
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
    } else if t == "io" {
      items.push(node(pos, mark(n, node-body(n, laplace)), stroke: none, name: name))
    } else {
      // gain, integrator, tf, function
      items.push(node(pos, mark(n, node-body(n, laplace)), shape: rect, name: name))
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
      items.push(edge(..verts, "-", stroke: none, label: text(0.9em, eval(e.sign, mode: "math")), label-pos: 0.92, label-side: left))
    }
  }

  diagram(
    node-stroke: 0.6pt,
    edge-stroke: 0.6pt,
    spacing: (1.8em, 1.4em),
    node-inset: 0.55em,
    ..items,
  )
}
