// Every generator as it is called from lecture notes.
#import "/typst/lecfig/lib.typ": *
#set page(width: 22cm, height: auto, margin: 1cm)

= `sp`
#sp("R_1 + (R_2 || C_1)")
#sp("U_0 + R_1 + (R_2 || (R_3 + L_1)) + C_1", loop: true)
#sp("j omega L || R || 1/(j omega C)".replace("1/(j omega C)", "Z_C"))

= `ode`
#ode("m y'' + d y' + c y = F", show-steps: true)
#ode("dot.double(x) + 2 D omega_0 dot(x) + omega_0^2 x = omega_0^2 u", blocks: "laplace")
#ode("T y' + y = K u")

= `logic`
#logic("y = (a and b) or not c")
#logic("s = a xor b xor c_\"in\"")

= `magnetic`
#magnetic("N_1 : N_2", fluxes: "split", ideal: true)
#magnetic("N", gap: "delta")

= `preset`
#preset("voltage-divider")
#preset("rc-lowpass")
#preset("inverting-amp")

= `loop`
#loop("feedback(C G, H)")
#loop("feedback(K feedback(G_1, H_1), H_2)", input: "w")
#loop("feedback([K_p (1 + 1/(T_n s))] G(s))", input: "w")
#loop("(G_1 + G_2) G_3")
