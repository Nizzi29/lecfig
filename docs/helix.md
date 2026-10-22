# lecfig in Helix

lecfig is driven from Helix with a few key bindings that run the `lecfig` CLI. Helix 25.01 or newer is needed for `%{buffer_name}` and `%{cursor_line}`.

## Setup

1. `just install` in this repository: builds the Typst plugin, links the package to `@local/lecfig:0.1.0`, installs the `lecfig` binary to `~/.cargo/bin`.
2. Once per course: `lecfig course init ~/path/to/course`. That creates `lecfig.toml` (marks the course root), `template.typ` and `figs/`.
3. Lecture files start with `#import "template.typ": *` (`lecfig lecture` writes that line for you).
4. Add to `~/.config/helix/config.toml` (the quotes keep paths with spaces in one piece):

```toml
[keys.normal.space.l]
n = ":insert-output lecfig new --for \"%{buffer_name}\""
b = ":insert-output lecfig new --kind block --for \"%{buffer_name}\""
p = ":insert-output lecfig new --preset ideal-transformer --for \"%{buffer_name}\""
e = ":run-shell-command lecfig edit --for \"%{buffer_name}\" --line %{cursor_line}"
```

## Use

| Keys | Effect |
| --- | --- |
| `space l n` | New circuit: inserts `#fig("fig-07")` at the cursor and opens the editor on it |
| `space l b` | New block diagram |
| `space l p` | New figure from a preset (edit the binding for other presets; `lecfig presets` lists them) |
| `space l e` | Reopen the figure whose `#fig("…")` is on the cursor line |

Generic boxes (converters, subsystems): `space b` places a labeled block. Labels are Typst math, so text goes in quotes and `\` breaks lines: `"AC/DC"`, `"DC" \ "DC"`. With the block selected, `+`/`-` and `>`/`<` resize it, `]`/`[` and `}`/`{` add or remove pins on the right and left.

Generic boxes (converters, subsystems): `space b` places a labeled block. Labels are Typst math, so text goes in quotes and `\` breaks lines: `"AC/DC"`, `"DC" \ "DC"`. With the block selected, `+`/`-` and `>`/`<` resize it, `]`/`[` and `}`/`{` add or remove pins on the right and left.

The editor runs in its own window and saves on every change; the tinymist preview updates as you draw. Close it with `q`; a figure left empty is deleted (remove its `#fig(…)` line too).

Generators work without the editor, straight in the notes:

```typst
#ode("m y'' + d y' + c y = F", show-steps: true)
#sp("R_1 + (R_2 || C_1)")
#logic("y = (a and b) or not c")
#magnetic("N_1 : N_2", fluxes: "split", ideal: true)
#loop("feedback(C G, H)", input: "w")
#preset("rc-lowpass")
```

To keep a generated figure and finish it with the mouse: `lecfig new --ode "…"` (or `--sp`, `--logic`, `--magnetic`, `--loop`) writes it to `figs/` and opens it in the editor; `e` in the editor changes the expression and regenerates.

## Other commands

| Command | Effect |
| --- | --- |
| `lecfig lecture --for FILE` | Creates today's `YYYY-MM-DD.typ` next to FILE |
| `lecfig mv old new` | Renames `figs/old.json` and rewrites `#fig("old"` and `@fig:old` in the course |
| `lecfig presets` | Lists the built-in presets and yours |
| `lecfig presets save NAME FIG` | Saves a figure as your preset (in `~/.config/lecfig/presets/`); then `lecfig new --preset NAME` or `space P` in the editor |
| `lecfig png figs/x.json out.png` | Renders a figure the way the editor sees it |
