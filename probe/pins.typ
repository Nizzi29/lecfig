// Dumps the anchors of every zap part lecfig supports, at angle 0:
//   typst eval --root . --in probe/pins.typ "query(<pin>).map(it => it.value)" > crates/core/src/pins.json
// Two-terminal parts are placed from (0, 0) to (2, 0); point parts at (0, 0).
#import "@preview/zap:0.6.0"

#let two = ("resistor", "capacitor", "inductor", "vsource", "isource", "acvsource", "dvsource", "disource", "battery", "diode", "led", "zener", "fuse", "switch", "lamp", "voltmeter", "ammeter", "heater", "potentiometer", "pcapacitor")
#let one = ("ground", "earth", "frame", "vcc", "vee", "opamp", "npn", "pnp", "nmos", "pmos", "land", "lor", "lxor", "lnand", "lnor", "lxnor", "lnot", "dflipflop", "jkflipflop", "srlatch", "transformer")
#let skip = ("start", "end", "mid", "symbol", "line", "bounds", "gl", "0", "1", "default", "shape", "bubble", "label")

#let dump(kind, args) = zap.circuit({
  import zap: *
  let f = dictionary(zap).at(kind)
  f("x", ..args)
  zap.cetz.draw.get-ctx(ctx => {
    let names = (ctx.nodes.at("x").anchors)(()).filter(a => a not in skip and not a.contains("-") and not a.starts-with("edge"))
    let pins = (:)
    for a in names {
      let (_, p) = zap.cetz.coordinate.resolve(ctx, "x." + a)
      pins.insert(a, (calc.round(p.at(0), digits: 4), calc.round(p.at(1), digits: 4)))
    }
    zap.cetz.draw.content((0, 0), [#metadata((type: kind, pins: pins)) <pin>])
  })
})

#for k in two { dump(k, ((0, 0), (2, 0))) }
#for k in one { dump(k, ((0, 0),)) }
