# Windows

Package `meshtown-windows`: the node with a tray icon. The package is a library (`gui/lib.rs`, the tray) and a binary
`meshtown-windows` (`cli/main.rs`). It depends on the `meshtown-core` crate in `core/` with the `gui` feature.

## Layout

- `cli/` — the binary entry point.
- `gui/` — the tray, clipboard and URL opening.
- `build/` — packaging: `build.sh` cross-builds `.exe` files in a container with `cargo-zigbuild`.

## Commands

Run from the repository root as `make windows-<target>`, or `make -C platforms/windows <target>`.

| Target | What it does |
|---|---|
| `build` | `cargo build -p meshtown-windows` |
| `test` | `cargo nextest run -p meshtown-windows` |
| `run` | `cargo run -p meshtown-windows -- $(ARGS)` |
| `package` | `platforms/windows/build/build.sh $(KIND)` |

`KIND` is `gui` (default) or `cli`. The release files go to `dist/` (`meshtown-windows-gui-<version>.exe`,
`meshtown-windows-cli-<version>.exe`). The root targets `make build_windows_cli` and `make build_windows_gui` call the
same script.

## What is used outside this directory

Only `core/` (the path dependency in `Cargo.toml`), `scripts/release/lib.sh`, `scripts/release/dist/` and the workspace root, from
which `cargo` runs. `make boundary` checks this.
