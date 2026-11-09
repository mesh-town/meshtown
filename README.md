# Mesh Town

**Reach your devices from anywhere — no port forwarding, no central server, no accounts.**

**Website: [mesh-town.github.io](https://mesh-town.github.io/)** — what it does, how it works and how to install, on one page.

Mesh Town connects your computers, phones and routers into a private network and lets you reach the services running
on them — SSH to a home server, RDP to the office machine, a NAS or router web panel — from a café, a train or
another country. A web interface opens by a link: with the browser set to use the node's proxy, a shared port is at
`http://<device-name>.meshtown`. No public IP to get, no routing to configure, no server to keep running, no account to
create in someone else's service.

Runs on macOS, Windows, Linux, Android, and OpenWrt, Keenetic or MikroTik routers.

**Contents:** [Who it is for](#who-it-is-for) · [Why it fits](#why-it-fits) · [How it differs](#how-it-differs) ·
[Not for you if](#not-for-you-if) · [Several networks](#several-networks-who-sees-what) · [Exit node](#also-use-a-member-as-an-exit-node) · [Quick start](#quick-start) ·
[Install](#install) · [Safety](#safety) · [Limitations](#limitations) · [Building](#building) ·
[Documentation](#documentation)

## Who it is for

- **Homelab enthusiasts** — a Raspberry Pi, a NAS or Home Assistant at home, behind a router you can't or don't want
  to open to the internet. Keep them in a network of your own, and share a single port with the family in another.
- **Self-hosters with friends** — a media library, a file share or a game server at home that a few friends
  should reach. Put the server and the friends in a network of their own; your other devices are not in it, so
  they do not exist for your friends. Each friend runs Mesh Town too.
- **Sysadmins** — a few machines at a client's office or at your relatives', each behind its own NAT. One network per
  client: you are an admin in each, the clients never see each other, and a lost laptop is removed in one click.
- **Small teams** — a dev database, an internal web tool or a build machine shared with colleagues, without a cloud
  service in the middle. A second admin keeps the network alive when the creator is away.

## Why it fits

- **No port forwarding, no NAT rules, no public IP.** Nodes find each other and connect directly; nothing on your
  router is opened to the internet. Where a direct connection is impossible, another member of your network
  passes the traffic on, encrypted end to end.
- **Keeps working when you change networks.** Wi-Fi, mobile data, a hotel network: the node notices the switch and
  reconnects to the other members within seconds. Connections open at that moment are closed and reopened by the
  program.
- **No central server.** Nodes meet on the public DHT, a distributed directory that nobody owns. Nothing for you to
  host, no server of ours your network depends on.
- **No accounts.** A network is a secret key and a member list signed by the members. A newcomer joins with a
  single-use invite from a member. No device limit.
- **No root or administrator rights to run.** No virtual network adapter, no kernel driver, no change to the system
  routes: the node is an ordinary program, and its proxy listens on a port above 1024. Only installing a system
  package (`apt`, the Windows installer) asks for rights once; on Android the app asks for the usual VPN permission.
- **You choose who sees what.** A node can be in several networks — "my devices", "family", "the team" — and a
  device exists only for the members of the networks it is in. Within a network, each port is shared with all
  members or with selected ones. Admins invite, remove members and make other admins.
- **Web interfaces open by a link.** With the browser using the node's proxy, a router, NAS or home automation panel
  is at `http://<device-name>.meshtown`, no IP addresses to remember.
- **A member can be your exit node.** Point a browser or a program at the local SOCKS5/HTTP proxy, and its
  traffic goes out to the internet through a trusted member's connection — see [below](#also-use-a-member-as-an-exit-node).

## How it differs

| The usual way | What it takes | With Mesh Town |
|---|---|---|
| Port forwarding and dynamic DNS | A public IP (not available behind a carrier's NAT), a rule on every router, the service open to the whole internet | Nothing to open: only members reach the port |
| Your own tunnel server | A machine with a public IP, its keys and routing, keeping it up | No server: members connect to each other directly |
| A tunnel or relay service | An account with the provider, your traffic through its servers | No account, no provider in the middle |
| A mesh network with a coordination server | An account or a self-hosted coordination server every device signs in to | The member list lives with the members |

What you give up: Mesh Town connects TCP and UDP program ports, not whole IP networks — see [Limitations](#limitations).

## Not for you if

- you need a whole subnet or a virtual network interface: Mesh Town shares TCP and UDP program ports running on
  the device itself;
- you need it on an iPhone or iPad without a router or a computer running it on the same Wi‑Fi;
- you want anonymity: the members you invite are trusted, and an exit sees which hosts you connect to through it.

## Several networks: who sees what

A node holds up to 8 networks at once; add one with **Add a network** on the page or `meshtown network create`. In
each network the node has its own identity and name, its own member list, shared ports and relay; members of one
network learn nothing about another — not the devices, not their names, not the ports.

- `home-lab`: the NAS, the home server and your laptop. Nobody else is in it.
- `family`: the same NAS, sharing one port — the photo library. The SSH port stays in `home-lab`.
- `friends`: the home server shares the media library on port 8096 and the game server's port; the friends'
  laptops are in `friends` only, and they see the server and those two ports — not the NAS, not SSH.
- `contractor`: your office PC and the contractor's laptop, one shared port. When the job ends, an admin removes the
  laptop; the key does not change, and your other networks never knew the contractor existed.

Whoever creates a network is its first admin. Only admins invite (single-use invites, valid for a day), remove a
member, make another member an admin or delete the network for everyone; make a second admin soon. Everyone else
sees the member list and uses the network. What is shared between a node's networks: the local proxy and its exit
choice. What is not: everything above.

## Also: use a member as an exit node

A member can let the others send traffic out through its internet connection. Point a browser or a program at the
local proxy `127.0.0.1:1080` (SOCKS5 and HTTP on one port) and its connections leave from that member's address — for
example, to reach a service that accepts connections only from your office or home IP. Any member can act as an exit
node, serving only members of its network. SOCKS5 carries UDP too (`UDP ASSOCIATE`), so QUIC and voice calls can
go out the same way.

## Quick start

1. [Install](#install) and start Mesh Town. Without arguments `meshtown` opens the tray app; in a terminal or on a
   server use `meshtown-cli`.
2. Open the node page from the tray menu, or with the sign-in link printed at start
   (`Mesh Town page: http://127.0.0.1:7777/?token=…`). Set a password when asked.
3. Press **Create a new network**, or **Join with an invite** if a member gave you one (in a terminal:
   `meshtown join mesh-invite2-…`). A node can be in several networks: **Add a network** on the page.
4. **Invite a member** on the page gives a single-use string for the next device or person. Invite from the tab of the network the person belongs to; only an admin can.
5. Share a port in the network's **Shared services (ports)** tab, or open one a member shared. With the browser using
   the proxy from step 6, a shared web interface is at `http://<device-name>.meshtown`; the tab shows the link.
6. To go out through a member, point a browser or a program at `127.0.0.1:1080` (`socks5h://` so the node resolves
   names): `curl -x socks5h://127.0.0.1:1080 https://example.com/`. Instead of typing the address, a browser can take
   the node's auto-config file, `http://127.0.0.1:1080/proxy.pac` (the setup assistant on the page asks whether to set
   it up for the whole system or for a browser). With `socks.strict` on, ordinary sites stay closed while the node is
   not running instead of opening directly ([docs/CONFIG-FILE.md](docs/CONFIG-FILE.md)).

Every option, the settings file and the node page: [docs/USAGE.md](docs/USAGE.md).

## Install

| System | Command or file |
|---|---|
| macOS | `brew install mesh-town/macos/meshtown`, or a zip from [meshtown-dist](https://github.com/mesh-town/meshtown-dist) |
| Windows | [meshtown-setup.exe](https://github.com/mesh-town/meshtown-dist/raw/main/windows/meshtown-setup.exe) (installer), or [meshtown.exe](https://github.com/mesh-town/meshtown-dist/raw/main/windows/meshtown.exe) (tray, no installation) or [meshtown-cli.exe](https://github.com/mesh-town/meshtown-dist/raw/main/windows/meshtown-cli.exe) |
| Linux | [x86_64](https://github.com/mesh-town/meshtown-dist/raw/main/linux/meshtown-linux-x86_64.tar.gz) · [aarch64](https://github.com/mesh-town/meshtown-dist/raw/main/linux/meshtown-linux-aarch64.tar.gz) |
| Debian, Ubuntu | [meshtown_amd64.deb](https://mesh-town.github.io/meshtown-dist/linux/debian/meshtown_amd64.deb) · [meshtown_arm64.deb](https://mesh-town.github.io/meshtown-dist/linux/debian/meshtown_arm64.deb), then `sudo apt install ./meshtown_<arch>.deb`; updates come with `apt upgrade` ([details](docs/INSTALL.md#debian-ubuntu-apt)) |
| Android | [meshtown.apk](https://github.com/mesh-town/meshtown-dist/raw/main/android/meshtown.apk) (arm64-v8a, Android 7.0 or newer) |
| OpenWrt | `wget -O- https://mesh-town.github.io/meshtown-openwrt/install.sh \| sh` |
| Keenetic (Entware) | `wget -O- https://mesh-town.github.io/meshtown-keenetic/install.sh \| sh` |
| MikroTik (RouterOS 7) | container image `ghcr.io/mesh-town/meshtown:latest` ([setup](docs/MIKROTIK.md)) |

On every system `meshtown` is the build with the tray icon and `meshtown-cli` the node in a terminal; they share
settings, so run one at a time. The macOS zip and the Windows binary are not notarized or signed, so the first launch
needs a confirmation. Gatekeeper, console-only Linux builds, router details, versioned archives and checksums:
[docs/INSTALL.md](docs/INSTALL.md).

## Safety

- **Members are trusted.** A member can use your shared ports and, if you are an exit, your connection. Invite only
  people you trust; a network admin removes a lost device, the network key does not need to change.
- **A network is a boundary.** A member of one of your networks sees nothing of the others. For a device only you
  should reach, use a network nobody else is in.
- **Keep the network key and `config.json` private.** The key alone does not let a node in, but it reveals the
  network's records in the DHT. Pass it through `MESHTOWN_KEY` or the settings file, not the command line.
- **An exit lends your IP address** and sees which hosts members connect to. Turn it off with `--no-exit` or on the
  node page.
- **The node page is plain HTTP** and by default reachable from your local network; set `meshtown access page localhost`
  on a network you do not trust. Never share the page port.

More in [SECURITY.md](SECURITY.md).

## Limitations

- TCP and UDP program ports on the device itself; no TUN, no subnet on desktops and routers. On Android the app
  takes the phone's traffic through the system VPN service and hands it to the node's proxy (tun2proxy).
- A SOCKS5/HTTP proxy, not a system-wide tunnel: the program must be able to use a proxy or connect to
  `127.0.0.1:<port>`.
- The exit is picked from a list you set, one hop, no traffic accounting; IPv6 only through an exit.
- Some mobile operators block direct connections between nodes.

Full list: [docs/LIMITATIONS.md](docs/LIMITATIONS.md). Releases of the 0.3.x line are a working network node, not a
hardened product.

## Building

Rust 1.94 or newer; Node 22.18 only to rebuild the node page.

```sh
cargo build --release --locked -p meshtown-<os>                                 # with the tray; <os> is macos, linux or windows
cargo build --release --locked --no-default-features --features net-hyper     # no tray, for servers
```

`make` prints the project command list; `make check` runs the tests and `clippy` as in CI. Release packages, containers
and the macOS bundle: [docs/BUILDING.md](docs/BUILDING.md).

## Documentation

- [docs/USAGE.md](docs/USAGE.md) — running the node, the settings file, invites, several networks, proxy and exit, relay, the node page, the log.
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — modules, protocols, data flow.
- [docs/CONFIG-FILE.md](docs/CONFIG-FILE.md) — the settings file.
- [docs/WEB-API.md](docs/WEB-API.md), [docs/WEB-API-config.md](docs/WEB-API-config.md),
  [docs/WEB-API-members.md](docs/WEB-API-members.md), [docs/WEB-API-embedding.md](docs/WEB-API-embedding.md) — the node page's HTTP API.
- [docs/INSTALL.md](docs/INSTALL.md), [docs/BUILDING.md](docs/BUILDING.md), [docs/LIMITATIONS.md](docs/LIMITATIONS.md).
- [CHANGELOG.md](CHANGELOG.md), [SECURITY.md](SECURITY.md), [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Credits and license

The DHT and transport come from [townjump](https://github.com/mesh-town/townjump), a Rust port of
[Holesail](https://github.com/holesail/holesail). The word list in network keys is the EFF Large Wordlist for
Passphrases (CC BY 3.0 US; the words are embedded in compressed form without the dice numbers, see
`core/members/wordlist/NOTICE.md`). The page is built with [Vue](https://vuejs.org/). Full list: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

AGPL-3.0-only, see [LICENSE](LICENSE). Contributions are accepted under Apache-2.0, see [CONTRIBUTING.md](CONTRIBUTING.md).
