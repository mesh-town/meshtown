# Linux

Package `meshtown-linux`: the node with a tray icon. The package is a library (`gui/lib.rs`, the tray) and a binary
`meshtown-linux` (`cli/main.rs`). It depends on the `meshtown-core` crate in `core/` with the `gui` feature.

## Layout

- `cli/` — the binary entry point.
- `gui/` — the tray, clipboard and URL opening.
- `build/` — packaging: `build.sh` cross-builds static binaries in a container; `keenetic/`, `openwrt/` and `mips/`
  hold the router targets, `debian/` the `.deb` package and the apt repository.

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

## Debian package

`build/debian/deb.py` builds `meshtown_<version>_<arch>.deb` (amd64, arm64) from the static release binary with the
Python standard library: the binary, a systemd unit that stays disabled until `systemctl enable`, and the apt source
and key of the repository (`keys/meshtown.gpg`). `build/debian/apt_repo.py update` adds packages to an apt repository
tree, prunes old versions, rebuilds the indexes with by-hash files and signs them with `gpg`.
`scripts/release/dist/build_debs.sh <assets> <out> <version>` builds both packages from the release archives, and
`scripts/release/dist/publish.sh` publishes them to `linux/debian/` of the distribution repository, served at
`https://mesh-town.github.io/meshtown-dist/linux/debian`. Tests: `make test-py` (the ones that need `dpkg-deb`,
`apt-get` or `gpgv` are skipped where these tools are missing).

## What is used outside this directory

Only `core/` (the path dependency in `Cargo.toml`), `scripts/release/lib.sh`, `scripts/release/dist/` and the workspace root, from
which `cargo` runs. `make boundary` checks this.
