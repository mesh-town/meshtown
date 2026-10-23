# Mesh Town

A private network for your home, your family or a small team. It connects your computers, phones and routers
directly to each other, so you can reach a service running on one device from another, and open sites through the
internet connection of a member you trust. There is no central server, no account to register, and no device quota.
It runs on macOS, Windows, Linux, Android and OpenWrt or Keenetic routers.

## What it is

Mesh Town is a peer-to-peer network of nodes. Each network has its own secret network key, and every node has its own
node key. Each node:

- finds the other members of its network on the public DHT, using only the network key (no server of ours is
  involved);
- lets in only the nodes that are on the network's member list: a member confirms a newcomer, and members can remove
  a node by vote. Knowing the network key alone is not enough to join;
- publishes the TCP ports of programs on its own computer to all members or to selected ones, and opens the ports
  other members shared with it on `127.0.0.1`;
- runs a local proxy (`127.0.0.1:1080`, SOCKS5 and HTTP on one port) and, if you allow it, works as an exit for other
  members;
- checks which sites open from its own connection and shares the results with the members;
- serves a small management page and, in the desktop build, shows a tray icon.

One node can be in several networks at once (up to 8), for example "my devices" and "friends".

This repository is the application: the Rust node, the management page (Vue, built into one committed file), the tray,
and the packaging scripts for macOS, Linux, Windows, Android and routers. The current line is 0.3.x.

## How it works

1. A network key looks like `our-gaming-room--finch-unwary-k7m2q9x4rb8vhn3tz01g5`: a free-form label plus a random
   part (two words from the EFF list and 21 characters, 130.8 bits). Everything else is derived from the random part.
2. Nodes find each other through the public HyperDHT, using an encrypted record that only holders of the network key
   can read. The DHT and the transport come from [townjump](https://github.com/mesh-town/townjump), a Rust port of
   Holesail.
3. A node becomes a member when it is on the member list. The list lives in the network document: a small record
   shared by all members, where every entry is signed by its author. It also holds the published ports, the site
   checks, the exit flags and the program versions; each node keeps its own domain list.
4. New members join with a single-use invite (valid for a day), and the inviting member admits them at once. A node
   that starts with the network key instead waits until a member confirms it on the node page.
5. A member opens a published port: the node listens on `127.0.0.1:<port>` and carries each connection to the
   member that published it.
6. A program that talks to the local proxy asks to connect to a domain. The node picks a path (itself, or a member
   chosen as an exit) and moves the bytes. An exit serves only members and only public addresses.

Modules and protocols are described in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Quick start

1. Install Mesh Town (see [Install](#install)).
2. Start it. Without arguments `meshtown` starts the tray app (desktop builds); in a terminal or on a server use
   `meshtown-cli` or `meshtown run`.
3. Open the node page from the tray menu, or with the sign-in link printed at start
   (`node page: http://127.0.0.1:7777/?token=… — sign-in link, valid for 10 minutes, single use`). Set a password
   when asked.
4. Press "Create a new network" on the page, or "Join with an invite" if a friend gave you one. In a terminal:
   `meshtown run --invite mesh-invite2-…`.
5. Invite the others: "Invite a member" on the page (or `invite` in the node console) gives a single-use string.
6. Share a port on the "Ports" tab, or open one that a member shared.
7. To open sites through a member, point a browser or a program at the proxy `127.0.0.1:1080` (SOCKS5; use
   `socks5h://` so the node resolves names), for example `curl -x socks5h://127.0.0.1:1080 https://example.com/`.

Every option, the settings file, the node page and the log format are described in [docs/USAGE.md](docs/USAGE.md).

## Safety

Read this before you invite anyone or let your node act as an exit. More in [SECURITY.md](SECURITY.md).

- **Members are trusted.** A member can use your published ports and, if you are an exit, your connection. Invite
  only people you trust. If a device is lost or a member leaves, the members vote that node out on the node page;
  the network key does not need to change.
- **Keep the network key secret anyway.** The key alone does not let a node in, but it lets anyone find the network's
  records in the DHT and see who its members are. Pass it through `MESHTOWN_KEY` or the settings file, not the
  command line.
- **An exit lends your IP address.** When you work as an exit, the sites a member opens and your ISP see that traffic
  as yours. Turn it off with `--no-exit` or on the node page if you do not want that.
- **Exits see where members connect.** An exit knows which hosts a member asks for. Mesh Town is not an anonymity
  tool; do not use it to hide who you are.
- **The node page uses plain HTTP.** By default (`localnet`) devices of your local network can open it. On a network
  you do not trust set `web.access` to `localhost` (`meshtown access localhost`). Never publish the page port.
- **Keep `config.json` private.** It holds the network key and the node secret.

## Install

### macOS

**Homebrew** (Apple silicon and Intel) is the easiest way:

```sh
brew install mesh-town/meshtown/meshtown
```

Then `meshtown` opens the tray app, and `meshtown` with arguments works as the command-line program. The formula also
installs `meshtown-cli`, the node in a terminal until Ctrl+C. To update: `brew upgrade meshtown`.

Homebrew 7 refuses formulas from a tap it does not trust yet; the full name above trusts the formula. If you prefer
`brew tap mesh-town/meshtown` first, run `brew trust mesh-town/meshtown` before `brew install meshtown`.

**Without Homebrew**, download [MeshTown-macos-arm64.zip](https://github.com/mesh-town/meshtown-dist/raw/main/macos/MeshTown-macos-arm64.zip)
(Apple silicon) or [MeshTown-macos-x86_64.zip](https://github.com/mesh-town/meshtown-dist/raw/main/macos/MeshTown-macos-x86_64.zip)
(Intel). The builds are signed ad hoc and are not notarized, so Gatekeeper blocks the first launch of a zip
downloaded in a browser. Either open **System Settings → Privacy & Security → "Open Anyway"**, or remove the
quarantine mark from a terminal:

```sh
xattr -dr com.apple.quarantine /path/to/MeshTown.app
```

Homebrew does not set this mark, which is why no window appears there.

### Windows, Linux, Android

Download from [meshtown-dist](https://github.com/mesh-town/meshtown-dist); the files without a version in the name are
always the latest build.

| System | File |
|---|---|
| Windows, with the tray icon | [meshtown.exe](https://github.com/mesh-town/meshtown-dist/raw/main/windows/meshtown.exe) |
| Windows, console only | [meshtown-cli.exe](https://github.com/mesh-town/meshtown-dist/raw/main/windows/meshtown-cli.exe) |
| Linux, x86_64 | [meshtown-linux-x86_64.tar.gz](https://github.com/mesh-town/meshtown-dist/raw/main/linux/meshtown-linux-x86_64.tar.gz) |
| Linux, aarch64 | [meshtown-linux-aarch64.tar.gz](https://github.com/mesh-town/meshtown-dist/raw/main/linux/meshtown-linux-aarch64.tar.gz) |
| Android | [meshtown.apk](https://github.com/mesh-town/meshtown-dist/raw/main/android/meshtown.apk) |

Console-only Linux builds are `meshtown-cli-linux-x86_64.tar.gz` and `meshtown-cli-linux-aarch64.tar.gz` in the same
folder. The folder of each system keeps the four latest versions too, with `SHA256SUMS` and notes. The Windows binary
is not signed; SmartScreen may warn on the first launch.

The same names on every system: `meshtown` is the build with the tray icon, `meshtown-cli` is the node in a terminal
until Ctrl+C. They share their settings, so do not run both at once.

### Routers

- **OpenWrt** (24.10 with `opkg` and 25.12 with `apk`). On the router, as root:

  ```sh
  wget -O- https://mesh-town.github.io/meshtown-openwrt/install.sh | sh
  ```

  The script finds the package manager of the router itself, adds the signed package feed and installs the package
  `meshtown`; the service starts on boot. Architectures: 64-bit ARM, 32-bit ARM Cortex and x86_64; MIPS, ARMv5/v6 and
  32-bit x86 routers are not covered yet.

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

### Release archives

[GitHub Releases](https://github.com/mesh-town/meshtown/releases) have versioned archives for macOS (`MeshTown.app`
in `MeshTown-macos-arm64-<version>.zip` and `MeshTown-macos-x86_64-<version>.zip`), Linux (x86_64 and aarch64, static
musl builds, `.tar.gz`), Windows (x86_64, `.zip`), console-only builds for 32-bit ARM and MIPS Linux, and
`SHA256SUMS`. Every platform has a `meshtown-cli-…` archive without the tray; the other archives are the tray builds.
Check an archive against `SHA256SUMS` (`shasum -a 256 -c SHA256SUMS --ignore-missing`) before running it.

The node page tells you when a newer version is out.

## Building

Requirements: Rust 1.94 or newer (see `rust-version` in `Cargo.toml`). Node 22.18 or newer is needed only to rebuild
the node page (`core/web_gui/ui/bundle/app.js` is committed). The code depends on townjump by a pinned git revision, so the
first build needs network access to GitHub.

```sh
cargo build --release --locked -p meshtown-<os>   # the binary: target/release/meshtown-<os> (with the tray); <os> is macos, linux or windows
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

- **No TUN and no capture of all system traffic.** Mesh Town gives a local SOCKS5/HTTP proxy and published TCP ports;
  a program must be able to use a proxy itself (`socks5h://`, a browser setting) or connect to `127.0.0.1:<port>`.
- **Published ports are TCP only**, and only for programs listening on the publishing computer itself.
- **No authentication in SOCKS5.** By default the proxy listens only on `127.0.0.1`.
- **UDP through the proxy only with `UDP ASSOCIATE`, and no `BIND`.** `UDP ASSOCIATE` works with an exit that
  supports it (a current build of Mesh Town); an exit without UDP support is skipped and the next one in
  `route.exits` is tried, and if none supports it the answer is code `0x07`, "command not supported". With no exits
  chosen, datagrams are sent directly from the node. Datagram fragmentation is not supported (such datagrams are
  dropped), `BIND` still gets code `0x07`, and an association without traffic for 10 minutes is closed.
- **IPv6 is partial.** A SOCKS5 request or an HTTP `CONNECT [v6]:port` with an IPv6 address works through an exit
  that has IPv6 connectivity. For a domain name an exit tries its IPv4 addresses first (up to four, one after
  another) and falls back to its IPv6 addresses (AAAA records) when none of them opens, so a name that has only
  AAAA records opens through an exit with IPv6 connectivity and is refused quickly through one without it. A node
  itself resolves names for direct connections to IPv4 addresses only, and the proxy listens on IPv4 only. An exit
  refuses IPv6 addresses in its own local network segment, as it does for private IPv4 ranges.
- **The exit is picked from a list you set.** The node uses the exits in the `route.exits` list, in order; it cannot
  choose an exit by country or operator.
- **A refusal is final.** The node moves to the next exit in the list only when the current one cannot be reached;
  if an exit answers with a refusal, the client gets it.
- **No traffic accounting or speed limits** per member, and no limit on simultaneous connections.
- **One hop only.** Client → your node → a member → the site; chains of two or more members are not supported.
- **No HTTP inspection inside the tunnel.** The node moves bytes and does not look into them, so there is no filter by
  path, headers or content.

Releases of the 0.3.x line are a working network node, not a hardened product. See [SECURITY.md](SECURITY.md) for the
scope and how to report a problem.

## Credits

- [townjump](https://github.com/mesh-town/townjump) is the Rust port of [Holesail](https://github.com/holesail/holesail)
  that provides the DHT and the transport (via the forked `peeroxide-dht` and `libudx`).
- The word list for the random part of the key is the EFF Large Wordlist for Passphrases, © Electronic Frontier
  Foundation, https://www.eff.org/dice, license CC BY 3.0 US. It is embedded unchanged; see
  `core/members/wordlist/NOTICE.md`.
- The page is built with [Vue](https://vuejs.org/) (MIT).

The full list is in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## License

AGPL-3.0-only, see [LICENSE](LICENSE).
