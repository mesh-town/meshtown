# Building from source

Requirements: Rust 1.94 or newer (see `rust-version` in `Cargo.toml`). Node 22.18 or newer is needed only to rebuild
the node page (`core/web_gui/ui/bundle/app.js` is committed). The code depends on townjump by a pinned git revision, so the
first build needs network access to GitHub.

```sh
cargo build --release --locked -p meshtown-<os>   # the binary: target/release/meshtown-<os> (with the tray); <os> is macos, linux or windows
cargo build --release --locked --no-default-features --features net-hyper   # no tray, for servers
```

`make` without arguments prints the list of project commands. The build logic for release packages is in
`platforms/<system>/build/build.sh`, and the macOS app bundle is described in [platforms/macos/build/README.md](../platforms/macos/build/README.md).

| Command | Target | Result in `dist/` |
|---|---|---|
| `make build` | the OS of this computer | native build |
| `make build_macos` / `make build_macos_app` | macOS, command line / `.app` with a tray (arm64) | `macos-arm64/MeshTown-<version>.app` |
| `make build_linux`, `make build_windows` | Linux or Windows x86_64, cross-built in a container | `meshtown-<os>-{cli,gui}-<version>` |
| `make build_all` | all three in turn | — |
| `make build_<os>_cli` / `make build_<os>_gui` | without / with a tray | `meshtown-<os>-{cli,gui}-<version>[.exe]` |

The containers need Apple `container`, or Docker: `make build_linux CONTAINER=docker`. Run `make check` to run all
tests together with `clippy`; the same checks run in CI.
