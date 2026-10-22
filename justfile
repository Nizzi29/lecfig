# Typst package version; must match typst/lecfig/typst.toml
version := "0.1.0"
pkg_dir := env("XDG_DATA_HOME", env("HOME") / ".local/share") / "typst/packages/local/lecfig"

# List the recipes
default:
    @just --list

# Build the Typst plugin (WebAssembly) and copy it into the package
plugin:
    cargo build -p lecfig-plugin --release --target wasm32-unknown-unknown
    cp target/wasm32-unknown-unknown/release/lecfig_plugin.wasm typst/lecfig/lecfig.wasm

# Link the package into Typst's local package folder and install the CLI
install: plugin
    mkdir -p "{{pkg_dir}}"
    ln -sfn "{{justfile_directory()}}/typst/lecfig" "{{pkg_dir}}/{{version}}"
    cargo install --path crates/lecfig

# Run the Rust tests and compile every Typst test file
test: plugin
    cargo test
    mkdir -p target/typst
    for f in tests/*/*.typ; do typst compile --root . "$f" "target/typst/$(basename "$f" .typ).pdf"; done
