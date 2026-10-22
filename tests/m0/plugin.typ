// M0 check: Typst loads the Rust plugin. Run `just plugin` first, then
// `typst compile --root . tests/m0/plugin.typ` from the repo root.
#import "/typst/lecfig/lib.typ": schema-version
Schema version from Rust: #schema-version()
