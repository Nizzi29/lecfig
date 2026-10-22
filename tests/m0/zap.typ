// M0 check: zap 0.6.0 compiles on Typst 0.15 and draws IEC symbols.
#import "@preview/zap:0.6.0"
#set page(width: auto, height: auto, margin: 1cm)

= Passive network
#zap.circuit({
  import zap: *
  vsource("v1", (0, 0), (0, 3), u: $u_1$, i: $i_1$)
  resistor("r1", (0, 3), (3, 3), label: $R_1$)
  capacitor("c1", (3, 3), (3, 0), label: $C$, u: $u_C$)
  wire("c1.out", "v1.in")
  node("n1", (3, 3))
  ground("g1", (0, 0))
})

= Logic gates (default variant)
#zap.circuit({
  import zap: *
  land("g1", (0, 0))
  lor("g2", (3, 0))
  lxor("g3", (6, 0))
  lnot("g4", (9, 0))
})

= Electronics
#zap.circuit({
  import zap: *
  opamp("o1", (0, 0))
  bjt("q1", (4, 0))
  mosfet("m1", (7, 0))
  diode("d1", (9, 0), (12, 0))
})

= Equivalent circuit parts
#zap.circuit({
  import zap: *
  dvsource("e1", (0, 0), (0, 3), u: $mu u_1$)
  disource("f1", (3, 0), (3, 3), i: $beta i_B$)
  resistor("z1", (5, 0), (5, 3), label: $Z$)
  vsource("v2", (7, 0), (7, 3), current: "ac", u: $hat(u)$)
})
