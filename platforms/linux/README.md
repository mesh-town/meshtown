# Linux

Package `meshtown-linux`: the node with a tray icon. The package is a library (`gui/lib.rs`, the tray) and a binary
`meshtown-linux` (`cli/main.rs`). It depends on the `meshtown-core` crate in `core/` with the `gui` feature.

## Layout

- `cli/` — the binary entry point.
- `gui/` — the tray, clipboard and URL opening.
- `build/` — packaging: `build.sh` cross-builds static binaries in a container; `keenetic/`, `openwrt/` and `mips/`
  hold the router targets.

## Commands

Run from the repository root as `make linux-<target>`, or `make -C platforms/linux <target>`.

| Target | What it does |
|---|---|
| `build` | `cargo build -p meshtown-linux` |
| `test` | `cargo nextest run -p meshtown-linux` |
| `run` | `cargo run -p meshtown-linux -- $(ARGS)` |
| `package` | `platforms/linux/build/build.sh $(KIND)` |

`KIND` is `gui` (default) or `cli`. `build.sh` also takes `arm64_cli` and `armv7_cli`. The release files go to
`dist/` (`meshtown-linux-gui-<version>`, `meshtown-linux-cli-<version>`, ...). The root targets
`make build_linux_cli`, `make build_linux_gui`, `make build_linux_arm64_cli`, `make build_linux_armv7_cli` call the
same script.

## What is used outside this directory

Only `core/` (the path dependency in `Cargo.toml`), `scripts/release/lib.sh`, `scripts/release/dist/` and the workspace root, from
which `cargo` runs. `make boundary` checks this.
