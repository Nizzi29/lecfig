// M0 check: fletcher 0.5.8 compiles on Typst 0.15; preview of an M4 integrator chain.
#import "@preview/fletcher:0.5.8" as fletcher: diagram, edge, node
#set page(width: auto, height: auto, margin: 1cm)

// m y'' + d y' + c y = F  ->  y'' = 1/m (F - d y' - c y)
#diagram(
  node-stroke: 0.6pt,
  edge-stroke: 0.6pt,
  spacing: (2em, 2em),
  node((0, 0), $F$, stroke: none, name: <in>),
  node((1, 0), [], shape: circle, radius: 0.5em, name: <sum>),
  node((2, 0), $1/m$, name: <gain>),
  node((3, 0), $integral$, name: <i1>),
  node((5, 0), $integral$, name: <i2>),
  node((7, 0), $y$, stroke: none, name: <out>),
  node((4, 1), $d$, name: <kd>),
  node((6, 2), $c$, name: <kc>),
  edge(<in>, <sum>, "-|>"),
  edge(<sum>, <gain>, "-|>"),
  edge(<gain>, <i1>, "-|>", $dot.double(y)$),
  edge(<i1>, <i2>, "-|>", $dot(y)$),
  edge(<i2>, <out>, "-|>"),
  edge((4, 0), <kd>, "-|>"),
  edge(<kd>, (1, 1), <sum>, "-|>", $-$, label-pos: 0.9),
  edge((6, 0), <kc>, "-|>"),
  edge(<kc>, (1, 2), <sum>, "-|>", $-$, label-pos: 0.95),
)
