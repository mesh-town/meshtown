# Mesh Town

Open sites that your own ISP blocks through the computers of people you trust: a small private network of friends or
family, a local SOCKS5/HTTP proxy, and a tray app that sets it all up in a few clicks.

## What it is

Mesh Town is a peer-to-peer network of nodes that share one secret network key. Each node:

- finds the other members of its network on the public DHT, using only the key (no server of ours is involved);
- proves to every neighbor that it knows the same key (a handshake), so strangers cannot join;
- checks which sites open from its own connection and shares the results with the members;
- runs a local proxy (`127.0.0.1:1080`, SOCKS5 and HTTP on one port) and, if you allow it, works as an exit for other
  members;
- serves a small management page and, in the desktop build, shows a tray icon.

This repository is the application: the Rust node, the management page (Vue, built into one committed file), the tray,
and the packaging scripts for macOS, Linux, Windows and OpenWrt. The 0.2.x line is the first public
release.

## How it works

1. A network key looks like `our-gaming-room--finch-unwary-k7m2q9x4rb8vhn3tz01g5`: a free-form label plus a random
   part (two words from the EFF list and 21 characters, 130.8 bits). Everything else is derived from the random part.
2. Nodes find each other through the public HyperDHT, using an encrypted record that only holders of the key can read.
   The DHT and the transport come from [townjump](https://github.com/mesh-town/townjump), a Rust port of
   Holesail.
3. A verified neighbor becomes a member. Members share a network document (a small record of the members' site checks,
   exit flags and program versions); each node keeps its own domain list.
4. A program that talks to the local proxy asks to connect to a domain. The node picks a path (itself, or a member
   chosen as an exit) and moves the bytes. An exit serves only verified members and only public addresses.
5. New members join with a single-use invite (valid for a day) or with the key itself.

Modules and protocols are described in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Quick start

1. Install Mesh Town (see [Install](#install)).
2. Create a network key: `meshtown keygen our gaming room`, or press "Create a new network" on the node page.
3. Start the node. Without arguments `meshtown` starts the tray app (desktop builds); in a terminal or on a server use
   `MESHTOWN_KEY="<key>" meshtown run`.
4. Open the node page. The address with a one-time token is printed at start
   (`node page: http://127.0.0.1:7777/?token=…`), and the tray menu opens it too. Set a password when asked.
5. Give a friend an invite (`invite` in the console, or "Invite a member" on the page). They run
   `meshtown run --invite mesh-invite2-…` or paste it on the page.
6. Point a browser or a program at the proxy `127.0.0.1:1080` (SOCKS5; use `socks5h://` so the node resolves names),
   for example `curl -x socks5h://127.0.0.1:1080 https://example.com/`.

Every option, the settings file, the node page and the log format are described in [docs/USAGE.md](docs/USAGE.md).

## Safety

Read this before you invite anyone or let your node act as an exit. More in [SECURITY.md](SECURITY.md).

- **The key is access.** Whoever has the network key can join, see the members and use them as exits. Share it, and
  invites, only with people you trust, and pass keys through `MESHTOWN_KEY` or the settings file, not the command line.
- **An exit lends your IP address.** When you work as an exit, the sites a member opens and your ISP see that traffic
  as yours. Turn it off with `--no-exit` or on the node page if you do not want that.
- **Exits see where members connect.** An exit knows which hosts a member asks for. Mesh Town is not an anonymity
  tool; do not use it to hide who you are.
- **The node page uses plain HTTP.** There is no TLS, so on a network you do not trust set `web.access` to
  `localhost` (`meshtown access localhost`). Never publish the page port through `ports.published`.
- **Keep `config.json` private.** It holds the network key and the node secret.

## Install

Pick one way.

- **Download** from [meshtown-dist](https://github.com/mesh-town/meshtown-dist): the files without a version in the
  name are always the latest build.

  | System | File |
  |---|---|
  | macOS, Apple silicon | [MeshTown-macos-arm64.zip](https://github.com/mesh-town/meshtown-dist/raw/main/macos/MeshTown-macos-arm64.zip) |
  | macOS, Intel | [MeshTown-macos-x86_64.zip](https://github.com/mesh-town/meshtown-dist/raw/main/macos/MeshTown-macos-x86_64.zip) |
  | Windows, with the tray icon | [meshtown.exe](https://github.com/mesh-town/meshtown-dist/raw/main/windows/meshtown.exe) |
  | Windows, console only | [meshtown-cli.exe](https://github.com/mesh-town/meshtown-dist/raw/main/windows/meshtown-cli.exe) |
  | Linux, x86_64 | [meshtown-linux-x86_64.tar.gz](https://github.com/mesh-town/meshtown-dist/raw/main/linux/meshtown-linux-x86_64.tar.gz) |
  | Linux, aarch64 | [meshtown-linux-aarch64.tar.gz](https://github.com/mesh-town/meshtown-dist/raw/main/linux/meshtown-linux-aarch64.tar.gz) |
  | Android | [meshtown.apk](https://github.com/mesh-town/meshtown-dist/raw/main/android/meshtown.apk) |

  Console-only Linux builds are `meshtown-cli-linux-x86_64.tar.gz` and `meshtown-cli-linux-aarch64.tar.gz` in the same
  folder. Older versions: the folder of your system in meshtown-dist keeps the four latest, with `SHA256SUMS` and notes.

- **GitHub Releases** (https://github.com/mesh-town/meshtown/releases): archives for macOS (`MeshTown.app` in a zip,
  `MeshTown-macos-arm64-<version>.zip` for Apple silicon and `MeshTown-macos-x86_64-<version>.zip` for Intel), Linux
  (x86_64 and aarch64, static musl builds, `.tar.gz`), Windows (x86_64, `.zip`), and `SHA256SUMS`. Every platform has a
  `meshtown-cli-…` archive without the tray; the other archives (`meshtown-<version>-<target>`, and the macOS zip) are
  the tray builds. The same names on every system: `meshtown` is the build with the tray icon, `meshtown-cli` is the
  node in a terminal until Ctrl+C. They share their settings, so do not run both at once. Check the archive against `SHA256SUMS`
  (`shasum -a 256 -c SHA256SUMS --ignore-missing`) before running it.
- **Homebrew** (macOS, Apple silicon):

  ```sh
  brew install mesh-town/meshtown/meshtown
  ```

  Homebrew 7 refuses formulas from a tap it does not trust yet; the full name above trusts the formula. If you prefer
  `brew tap mesh-town/meshtown` first, run `brew trust mesh-town/meshtown` before `brew install meshtown`. The command
  `meshtown` without arguments opens the tray app; with arguments it works as the command-line program.
  The formula also installs `meshtown-cli` (see the rule above).

macOS builds are signed ad hoc and are not notarized (there is no Apple Developer ID certificate). A zip downloaded in
a browser carries the `com.apple.quarantine` mark, and Gatekeeper blocks the first launch. Either open **System
Settings → Privacy & Security → "Open Anyway"**, or remove the mark from a terminal:

```sh
xattr -dr com.apple.quarantine /path/to/MeshTown.app
```

Installing through Homebrew does not set the mark, so no window appears. The Windows binary is not signed either;
SmartScreen may warn on the first launch.

- **OpenWrt** (router; OpenWrt 24.10 with `opkg` and 25.12 with `apk`). On the router, as root:

  ```sh
  wget -O- https://mesh-town.github.io/meshtown-openwrt/install.sh | sh
  ```

  The script finds the package manager of the router itself, adds the signed package feed and installs the package
  `meshtown`; the service starts on boot. Architectures: ARM (32 and 64 bit) and x86_64; MIPS routers are not covered
  yet.

  To update, run the command for your OpenWrt version only (`grep DISTRIB_RELEASE /etc/openwrt_release` shows it):

  - OpenWrt 24.10 (`opkg`): `opkg update && opkg upgrade meshtown`
  - OpenWrt 25.12 (`apk`): `apk update && apk upgrade meshtown`

  Manual installation and details: [platforms/linux/build/openwrt/README.md](platforms/linux/build/openwrt/README.md).

- **Keenetic** (router with Entware installed; MIPS and aarch64 models). Over SSH on the router:

  ```sh
  wget -O- https://mesh-town.github.io/meshtown-keenetic/install.sh | sh
  ```

  The script reads the Entware architecture, adds the package feed and installs the package `meshtown`; the service
  `/opt/etc/init.d/S99meshtown` starts on boot. To update: `opkg update && opkg upgrade meshtown`. Not tested on a live
  KeeneticOS router yet.

## Building

Requirements: Rust 1.94 or newer (see `rust-version` in `Cargo.toml`). Node 22.18 or newer is needed only to rebuild
the node page (`core/web_gui/ui/bundle/app.js` is committed). The code depends on townjump by a pinned git revision, so the
first build needs network access to GitHub.

```sh
cargo build --release --locked          # the binary: target/release/meshtown (with the tray)
cargo build --release --locked --no-default-features --features net-hyper   # no tray, for servers
```

`make` without arguments prints the list of project commands. The build logic for release packages is in
`platforms/<system>/build/build.sh`, and the macOS app bundle is described in [platforms/macos/build/README.md](platforms/macos/build/README.md).

| Command | Target | Result in `dist/` |
|---|---|---|
| `make build` | the OS of this computer | native build |
| `make build_macos` / `make build_macos_app` | macOS, command line / `.app` with a tray (arm64) | `macos-arm64/MeshTown-<version>.app` |
| `make build_linux`, `make build_windows` | Linux or Windows x86_64, cross-built in a container | `meshtown-<os>-{cli,gui}-<version>` |
| `make build_all` | all three in turn | — |
| `make build_<os>_cli` / `make build_<os>_gui` | without / with a tray | `meshtown-<os>-{cli,gui}-<version>[.exe]` |

The containers need Apple `container`, or Docker: `make build_linux CONTAINER=docker`. Run `make check` to run all
tests together with `clippy`; the same checks run in CI.

## Documentation

- [docs/USAGE.md](docs/USAGE.md): running the node, the settings file, invites, proxy and exit, site checks, the node page and
  the log format.
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md): modules, protocols, data flow.
- [docs/CONFIG-FILE.md](docs/CONFIG-FILE.md): the settings file, its fields and versions.
- [docs/WEB-API.md](docs/WEB-API.md), [docs/WEB-API-config.md](docs/WEB-API-config.md), [docs/WEB-API-embedding.md](docs/WEB-API-embedding.md): the
  HTTP API of the node page.
- [CHANGELOG.md](CHANGELOG.md), [SECURITY.md](SECURITY.md), [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Limitations

- **No TUN and no capture of all system traffic.** Mesh Town gives only a local SOCKS5/HTTP proxy; the program must be
  able to use a proxy itself (`socks5h://`, a browser setting).
- **No authentication in SOCKS5.** The proxy listens only on `127.0.0.1`.
- **UDP only through `UDP ASSOCIATE`, and no `BIND`.** `UDP ASSOCIATE` works with an exit that supports it (a
  current build of Mesh Town); an exit without UDP support is skipped and the next one in `route.exits` is tried,
  and if none supports it the answer is code `0x07`, "command not supported". With no exits chosen, datagrams are sent
  directly from the node. Datagram fragmentation is not supported (such datagrams are dropped), `BIND` still gets
  code `0x07`, and an association without traffic for 10 minutes is closed.
- **IPv6 is partial.** A SOCKS5 request or an HTTP `CONNECT [v6]:port` with an IPv6 address works through an exit
  that has IPv6 connectivity. For a domain name an exit tries its IPv4 addresses first (up to four, one after
  another) and falls back to its IPv6 addresses (AAAA records) when none of them opens, so a name that has only
  AAAA records opens through an exit with IPv6 connectivity and is refused quickly through one without it. A node
  itself resolves names for direct connections to IPv4 addresses only, and the proxy listens on IPv4 only. An exit
  refuses IPv6 addresses in its own local network segment, as it does for private IPv4 ranges.
- **The exit is picked from a list you set.** The node uses the exits in the `route.exits` list, in order; it cannot
  choose an exit by country or operator.
- **No retry through another exit.** If the chosen member refuses, the client gets the refusal.
- **No traffic accounting or speed limits** per member, and no limit on simultaneous connections.
- **One hop only.** Client → your node → a member → the site; chains of two or more members are not supported.
- **No HTTP inspection inside the tunnel.** The node moves bytes and does not look into them, so there is no filter by
  path, headers or content.

Releases of the 0.2.x line are a working network node, not a hardened product. See [SECURITY.md](SECURITY.md) for the
scope and how to report a problem.

## Credits

- [townjump](https://github.com/mesh-town/townjump) is the Rust port of [Holesail](https://github.com/holesail/holesail)
  that provides the DHT and the transport (via the forked `peeroxide-dht` and `libudx`).
- The word list for the random part of the key is the EFF Large Wordlist for Passphrases, © Electronic Frontier
  Foundation, https://www.eff.org/dice, license CC BY 3.0 US. It is embedded unchanged; see
  `core/wordlist/NOTICE.md`.
- The page is built with [Vue](https://vuejs.org/) (MIT).

The full list is in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## License

AGPL-3.0-only, see [LICENSE](LICENSE).
