// Course template. Lecture files start with `#import "template.typ": *`.
#import "@local/lecfig:0.1.0" as lecfig
#import lecfig: logic, loop, magnetic, ode, preset, render, sp

/// A figure stored in figs/<name>.json: numbered, referable as @fig:<name>.
/// `lecfig new` inserts these lines; `lecfig edit <name>` reopens the figure.
#let fig(name, caption: none) = [#figure(
    lecfig.render(json("figs/" + name + ".json")),
    caption: caption,
  ) #label("fig:" + name)]
