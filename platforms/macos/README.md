# macOS

Package `meshtown-macos`: the node with a menu bar (tray) icon. The package is a library (`gui/lib.rs`, the tray) and a
binary `meshtown-macos` (`cli/main.rs`). It depends on the `meshtown-core` crate in `core/` with the `gui` feature.

## Layout

- `cli/` — the binary entry point.
- `gui/` — the tray, clipboard and URL opening.
- `build/` — packaging: `build.sh` builds the binaries and the `.app`, `app.sh` assembles the bundle, plus the icon
  source and generator. Bundle details and test signing: `platforms/macos/build/README.md`.

## Commands

Run from the repository root as `make macos-<target>`, or `make -C platforms/macos <target>`.

| Target | What it does |
|---|---|
| `build` | `cargo build -p meshtown-macos` |
| `test` | `cargo nextest run -p meshtown-macos` |
| `run` | `cargo run -p meshtown-macos -- $(ARGS)` |
| `package` | `platforms/macos/build/build.sh $(KIND)` |

Variables: `KIND` is `app` (default), `app_x86`, `app_universal` or `cli`; `ARGS` goes to the binary; `BUILD_ROOT`
(default `.tmp/build`) and `TARGET_DIR` (default `$(BUILD_ROOT)/host`) set the build directories; `DYN=0` turns off
dynamic linking used for dev and test builds. The release files go to `dist/`. The root targets
`make build_macos_cli`, `make build_macos_app`, `make build_macos_app_x86`, `make build_macos_app_universal` call the
same script.

## What is used outside this directory

Only `core/` (the path dependency in `Cargo.toml`), `release/lib.sh`, `release/dist/` and the workspace root, from
which `cargo` runs. `make boundary` checks this.
