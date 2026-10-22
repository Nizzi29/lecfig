// lecfig: circuits, block diagrams and magnetic figures for lecture notes.
//
//   #import "@local/lecfig:0.1.0": *
//   #render(json("figs/rc.json"))            a stored figure
//   #sp("R_1 + (R_2 || C_1)")                 series/parallel network
//   #ode("m y'' + d y' + c y = F")            ODE as block diagram
//   #logic("(a and b) or not c")              logic circuit
//   #magnetic("N_1 : N_2")                    transformer / magnetic circuit
//   #loop("feedback(C G, H)")                 control loop from block algebra
//   #preset("ideal-transformer")              a named preset
//
// Only render-circuit.typ and render-block.typ may import zap or fletcher.
#import "render-circuit.typ": render-circuit
#import "render-block.typ": render-block
#import "render-magnetic.typ": render-magnetic

#let _plugin = plugin("lecfig.wasm")

/// Schema version reported by the Rust plugin; proves the plugin loads.
#let schema-version() = int(str(_plugin.schema_version()))

/// Draws figure data: a dictionary as read by `json(...)` from a figure file.
/// `editor: true` adds invisible markers the lecfig editor uses to map pixels.
#let render(data, editor: false) = {
  let empty = (
    data.at("elements", default: ()).len() + data.at("wires", default: ()).len() + data.at("nodes", default: ()).len()
      + data.at("annotations", default: ()).len()
  ) == 0
  if empty and data.kind in ("circuit", "block") and not editor {
    // Placeholder while the figure is being drawn in the editor.
    return box(stroke: (paint: luma(170), dash: "dashed"), inset: 0.8em, text(0.85em, fill: luma(120))[figure in progress])
  }
  if data.kind == "circuit" {
    render-circuit(data, editor: editor)
  } else if data.kind == "block" {
    render-block(data, editor: editor)
  } else if data.kind == "magnetic" {
    render-magnetic(data, editor: editor)
  } else {
    panic("lecfig: unknown figure kind '" + data.kind + "'")
  }
}

/// Runs a generator and returns the figure data (not drawn), e.g. to inspect it.
#let generate(generator, expr, ..options) = json(_plugin.generate(
  bytes(generator),
  bytes(expr),
  bytes(json.encode(options.named(), pretty: false)),
))

/// Figure data of a named preset.
#let preset-data(name) = json(_plugin.preset(bytes(name)))

#let sp(expr, ..options) = render(generate("sp", expr, ..options))
#let logic(expr, ..options) = render(generate("logic", expr, ..options))
#let magnetic(expr, ..options) = render(generate("magnetic", expr, ..options))
#let loop(expr, ..options) = render(generate("loop", expr, ..options))
#let preset(name) = render(preset-data(name))

/// ODE as block diagram. `show-steps: true` puts the rearranged equation next to it.
#let ode(expr, show-steps: false, ..options) = {
  let data = generate("ode", expr, ..options)
  if show-steps and "steps" in data.source.at("options", default: (:)) {
    grid(
      columns: 2,
      gutter: 1.5em,
      align: horizon,
      render(data),
      eval(data.source.options.steps, mode: "math"),
    )
  } else {
    render(data)
  }
}
