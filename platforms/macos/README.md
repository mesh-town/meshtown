# macOS

Package `meshtown-macos`: the node with a menu bar (tray) icon. The package is a library (`gui/lib.rs`, the tray) and a
binary `meshtown-macos` (`cli/main.rs`). It depends on the `meshtown-core` crate in `core/` with the `gui` feature.

## Layout

- `cli/` — the binary entry point.
- `gui/` — the tray, clipboard and URL opening.
- `build/` — packaging: `build.sh` builds the binaries and the `.app`, `app.sh` assembles the bundle, plus the icon
  source and generator. Bundle details: `platforms/macos/build/README.md`.

## Commands

Run from the repository root as `make macos-<target>`, or `make -C platforms/macos <target>`.

| Target | What it does |
|---|---|
| `build` | `cargo build -p meshtown-macos` |
| `test` | `cargo nextest run -p meshtown-macos` |
| `run` | `cargo run -p meshtown-macos -- $(ARGS)` |
| `package` | `platforms/macos/build/build.sh $(KIND)` |

| Variable | Meaning |
|---|---|
| `KIND` | `app` (default), `app_x86`, `app_universal` or `cli` |
| `ARGS` | passed to the binary |
| `BUILD_ROOT` | build directory (default `.tmp/build`) |
| `TARGET_DIR` | cargo target directory (default `$(BUILD_ROOT)/host`) |
| `DYN` | `DYN=0` turns off the dynamic linking used for dev and test builds |

The release files go to `dist/`. The root targets `make build_macos_cli`, `make build_macos_app`,
`make build_macos_app_x86`, `make build_macos_app_universal` call the same script.

## External dependencies

`make boundary` checks that this directory uses only:

- `core/` (the path dependency in `Cargo.toml`);
- `scripts/release/lib.sh` and `scripts/release/dist/`;
- the workspace root, from which `cargo` runs.
