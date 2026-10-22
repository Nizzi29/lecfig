// Spike for the magnetic figure kind (docs/plan.md, "Magnetic figures").
// Each figure is written out as the data its generator call would return;
// the heading above it is that call.
#import "/typst/lecfig/render-magnetic.typ": render-magnetic
#set page(width: auto, height: auto, margin: 1cm)

= `magnetic("N_1 : N_2", fluxes: "split", ideal: true)`
// Ideal transformer: consumer arrows on the primary, generator arrows on the
// secondary, each winding's flux split into the part linking the other winding.
#render-magnetic((
  version: 1,
  kind: "magnetic",
  source: (generator: "magnetic", expr: "N_1 : N_2", fluxes: "split", ideal: true),
  core: (shape: "ring", size: (6, 3.7), leg: 0.9, yoke: 0.6, label: "mu"),
  windings: (
    (id: "w1", leg: "left", sense: "cw", turns: 3, label: "N_1", port: (i: "i_1", u: "u_1")),
    (id: "w2", leg: "right", sense: "cw", turns: 3, label: "N_2", port: (i: "i_2", i-dir: "out", u: "u_2")),
  ),
  fluxes: (
    (id: "f1", path: "loop", lane: 0.45, dir: "cw", arrows: ((leg: "top", pos: 0.75),)),
    (id: "f2", path: "loop", lane: -0.3, dir: "ccw", arrows: ((leg: "bottom", pos: 0.5),)),
    (id: "psi1", path: "left", lane: 0.1, dir: "cw", span: (0.15, 0.9), label: "Psi_1"),
    (id: "psi2", path: "right", lane: 0.1, dir: "cw", span: (0.15, 0.9), label: "Psi_2"),
  ),
  sections: ((id: "s1", leg: "right", pos: 0.96),),
  annotations: (
    (type: "callout", tone: "magnetic", content: "Phi_21 = Phi_11", to: (flux: "f1", leg: "top", pos: 0.6), rel: (-1, 1.1)),
    (type: "callout", tone: "magnetic", content: "Phi_12 = Phi_22", to: (flux: "f2", leg: "bottom", pos: 0.4), rel: (-0.5, -1.4)),
    (type: "callout", tone: "magnetic", content: "Phi_h = Phi_21 - Phi_12", to: "s1.north", rel: (0.4, 1.2)),
    (type: "callout", content: "R_1 = 0", to: "w1.lead2", rel: (-0.3, -0.9)),
    (type: "callout", content: "R_2 = 0", to: "w2.lead2", rel: (0.5, -0.9)),
    (type: "callout", content: "P_\"Fe\" = 0", to: (leg: "bottom", pos: 0.85, lane: 1), rel: (0.4, -0.9)),
  ),
))

= `magnetic("N_1 : -N_2")`
// Opposite winding sense on the secondary; one main flux.
#render-magnetic((
  version: 1,
  kind: "magnetic",
  source: (generator: "magnetic", expr: "N_1 : -N_2"),
  core: (shape: "ring", size: (6, 3.7), leg: 0.9, yoke: 0.6, label: "mu"),
  windings: (
    (id: "w1", leg: "left", sense: "cw", label: "N_1", port: (i: "i_1", u: "u_1")),
    (id: "w2", leg: "right", sense: "ccw", label: "N_2", port: (i: "i_2", i-dir: "out", u: "u_2")),
  ),
  fluxes: (
    (id: "f1", path: "loop", lane: 0, dir: "cw", arrows: ((leg: "top", pos: 0.6), (leg: "bottom", pos: 0.4))),
  ),
  annotations: (
    (type: "callout", tone: "magnetic", content: "Phi_h", to: (flux: "f1", leg: "top", pos: 0.8), rel: (0.6, 0.8)),
  ),
))

= `magnetic("N", gap: "delta")`
// Magnetic circuit: one winding, an air gap in the right leg.
#render-magnetic((
  version: 1,
  kind: "magnetic",
  source: (generator: "magnetic", expr: "N", gap: "delta"),
  core: (
    shape: "ring",
    size: (5, 3.4),
    leg: 0.8,
    yoke: 0.8,
    label: "mu_r",
    gaps: ((leg: "right", pos: 0.5, width: 0.25),),
  ),
  windings: ((id: "w1", leg: "left", turns: 4, label: "N", port: (i: "i", u: "u")),),
  fluxes: ((id: "f1", path: "loop", lane: 0, dir: "cw", arrows: ((leg: "top", pos: 0.5),)),),
  annotations: (
    (type: "callout", content: "delta", to: (leg: "right", pos: 0.5, lane: 1), rel: (0.8, 0.6)),
    (type: "callout", tone: "magnetic", content: "Phi", to: (flux: "f1", leg: "bottom", pos: 0.5), rel: (0.6, -1)),
  ),
))
