# lecfig

Circuits, logic diagrams, block diagrams and magnetic figures (transformers) for Typst lecture notes, fast enough to keep up with a lecture. Figures are stored as JSON, drawn by a Typst package (zap for circuits, fletcher for block diagrams, cetz for magnetic figures), and edited in a keyboard-first editor that shows the real Typst render.

```sh
just install                      # plugin, package link (@local/lecfig:0.1.0), lecfig binary
lecfig course init ~/notes/et2    # once per course
lecfig new --for ~/notes/et2/2026-10-16.typ   # prints #fig("fig-01"), opens the editor
```

In the notes, generators work without the editor:

```typst
#import "template.typ": *
#ode("m y'' + d y' + c y = F", show-steps: true)
#sp("R_1 + (R_2 || C_1)")
#logic("y = (a and b) or not c")
#magnetic("N_1 : N_2", fluxes: "split", ideal: true)
```

Helix setup and key bindings: `docs/helix.md`. Plan and status: `docs/plan.md`.

License: MIT (see `LICENSE`).
