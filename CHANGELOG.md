# Changelog

All notable changes are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/).

## Unreleased

### Added

- Tray: at start, a start-at-login entry that leads to another copy of the program is rewritten to the running one.
- Exit: a per-stream cap on data not yet acknowledged by the client (`exit.stream_window_kib`; 128 KiB on machines with up to
  128 MiB of RAM, off otherwise), so a few fast downloads cannot fill a small router's memory.
- Exit: the optional `exit.interface` setting sends a node's exit traffic (sites, UDP, its DNS) only through the chosen
  interface, for example another VPN. If the interface is missing or down, exit connections are refused (reply code 8)
  instead of using the regular network.
- Relay: a member's relay now pairs only members of its own network, so nobody outside the network can use it.
- Relay: a member's relay key is the same after every restart.
- Relay: a member advertises itself as a relay, together with its public address, only after it has confirmed that the
  address is reachable from outside; otherwise it closes the relay and tries again later.
- Relay: the receiving side also shows a relayed neighbour as `relay` in the node state.

### Changed

- The Homebrew tap is renamed to `mesh-town/homebrew-macos`: install with `brew install mesh-town/macos/meshtown`.
  An existing `mesh-town/meshtown` tap keeps updating through the GitHub redirect.

## 0.3.178 — 2026-10-27

### Changed

- The proxy auto-config (PAC) file now ends the route of ordinary sites with `DIRECT`, so they open directly while the
  node is not running. The new setting `socks.strict` (Settings, "Strict mode") turns this fallback off.
- Setup asks whether the proxy applies to the whole system or to the browser only, and asks about strict mode.

### Added

- Relay through members: when a direct connection does not come up, a node dials through a relay run by another member
  of the same network that has a public address. Members with a public address serve as relays by default. The per-network
  setting `net.relay` (`network` or `off`) controls both calling and serving.
- Peers reached through a relay show up as `relay` in the node state.
- The node page also opens at its `.meshtown` name in a browser on the same computer.
- A node that was excluded from a network learns it was removed and requests a new key when refused as excluded.
- Tray: quick quit while the node is starting, Cmd-Q stops the node, the menu refreshes when opened, network submenus
  stay visible while every network is offline.
- After a failed start the node stays alive and the page shows the reason, with a restart button and a closed-network hint.

### Fixed

- Joining right after adding a network slot, repeated invites reuse the waiting slot, and opened UDP ports are
  reapplied when the list is unchanged.
- The admission banner is shown once and stays across Home tabs; invite error texts are clearer; a port change can be
  cancelled; the UDP port reports "binding" until it is bound.
- Internal: page layout, wording and tray text polish.

## 0.3.163 — 2026-10-26

### Changed

- **Incompatible with earlier 0.3.x networks.** Peer-voted networks are gone: every new network is managed, its creator
  is the only administrator, and the page suggests naming a second one. Keys of the older format (`mesh1-…`, or three
  words with a group) and invites of the older format are refused with a message to ask an administrator for a new
  invite. A node whose main network key is an old one starts without a network and logs why.
- The Networks tab shows Members, Shared services and Device settings per network.

### Removed

- The network type picker, voting and the `kind` field of network creation.

### Fixed

- Broken network slots, role display, hints and a narrow ports table on the network page and in the tray.
- Tray keeps the network submenus while every network is offline.

## 0.3.159 — 2026-10-26

### Fixed

- The build for MIPS routers (portable 64-bit atomics in the exit limits).

## 0.3.158 — 2026-10-26

### Added

- Exit: a stream limit and memory tiers so that small routers are not overloaded.
- Releases are also published to the distribution repository.

## 0.3.150 — 2026-10-26

### Changed

- The `--relay-only` and `--no-relay` flags and the `transport.mode` setting are removed. An old file that still has
  `transport.mode` starts as before: the key is ignored, a warning goes to the log, and the key disappears on the next
  write.
- Invites and member list: an excluded node does not hand out a key, an excluded node that restarted can be readmitted,
  an exclusion stays in force when the network grows, and invites carry up to four direct addresses.
- Connections: a direct dial by a member's address, LAN candidates are tried alongside hole punching on a shared public
  address, and the link round-trip time is logged.
- The word "computer" replaces "device" and "phone" wording across the page and tray.
- The binary is smaller (the page is compressed at build time).
- Proxy: a deadline for the head of a request, SOCKS5 reply 7 only when every exit lacks UDP support, broken pooled
  exits are dropped.
- The node starts with its version first in the log; the CLI help describes the product.

### Added

- Invites show as a QR code; Android scans a QR code (also rotated or skewed ones from a photo).
- Ports page: publish and open UDP ports.
- Android: node log level from a `rust-log` file.
- Page: an admission-refused banner with a red header, remembered exit order, a Home that fits the first screen.
- Tray: the hidden-icon check and dialog, notifications through the system notification center, optimistic exit check
  marks, an unnamed network is named like on the page.
- Typed refusal of an out-of-network invite and a redial after a local refusal; a woken redial for a member who is
  allowed again.

### Fixed

- Stopping the node interrupts startup, the exit drain on stop is bounded, stop signals are unblocked at start, and the
  status shows the live member count.
- Config saves are serialized; spacing and disabled buttons on the page.

## 0.3.116 — 2026-10-24

### Changed

- Joining by invite enters the network the invite was made for; a later administrator invite brings an excluded member
  back into a managed network; invites of a demoted administrator stop working.
- The access code is shorter.
- Managed networks show on the page, in the tray and on Android; administrator roles, invites from administrators and
  network deletion work.
- Several networks on one page: network tabs, overview polling and shared settings; the exits of every network are
  listed on Home and one can be chosen in any of them; a removed member disappears from the peer list at once.
- Refusals are shown next to the join button and cleared on a new attempt.

### Added

- Tray: open the page from Add and notice items, and a notification when an invite depends on this computer.
- Linux: a `.deb` package and a signed apt repository.
- Android: the update downloads inside the app and opens the system installer, the downloaded update is checked first,
  the node restarts after an update; the version code follows the node version, target SDK 35, a battery exemption prompt.
- Serving `<name>.meshtown` addresses through the node proxy; the member web address on the Ports tab; shared-with-you
  ports first, with Copy and Change number for an opened port.
- A pending network join can be cancelled, and the page says when nobody answers.
- The DHT routing table is kept on disk and seeded on start.
- Entware and OpenWrt: CLI commands find the service settings.

### Fixed

- A second node with the same settings is refused before it takes any port.
- Exit choice applies in the tab of the network it is pressed in; stale remembered exits are dropped and removed exits
  are marked.
- The invite form is shown to a computer removed from the member list.
- Ports opened in another network are refused; launch flags reach added networks.

## 0.3.96 — 2026-10-24

### Changed

- **Incompatible with data from before 0.3.71.** Compatibility with the old invite, storage and config formats is
  removed, and the old record format is dropped; use fresh invites and recreate such files.
- Several networks in one node: per-network files and saved network slots, the CLI and API add a network, a network added
  from the command line is seeded, an invite is redeemed for a new slot with the slot's own secret.
- One proxy front per process; the network label appears in the log.
- Members can be removed from a network without a ban.

### Added

- SOCKS5 UDP ASSOCIATE and IPv6 targets; IPv6-only names are resolved on the exit.
- Tray menu per network; Android status, menu and reset polish.

### Fixed

- Offline members are removable from the members page.
- Internal: workspace layout, scripts grouping, dead-code removal, test cleanups.

## 0.3.42 — 2026-10-22

### Changed

- macOS builds run on newer runners.

## 0.3.0

### Changed

- **Incompatible with 0.2.x.** The protocol identifiers, the DHT keys, the gossip topic and the shared-record
  namespace are renamed, so new nodes and 0.2.x nodes do not connect and do not find each other. Update every node of
  a network at once.
- Invitations already issued, invite holds and the owner's invite signatures stop being valid; issue new invitations.
- Default node names change (the same node key now yields a different name).
- A network created from a label with the word tail (`label--word-word-group`) is no longer recognised from the
  tail alone: after the update the creator takes the machine form `mesh1-…` from a member and enters it. The personal
  domain list is lost: the old `domains.bin` is moved to quarantine.
- The built-in geo-block markers are English only. Add other languages to `checks.geo_markers` in the node's settings
  file (see `CONFIG-FILE.md`).

### Added

- Peer-to-peer network of nodes sharing a secret key: discovery through the public DHT (townjump), a handshake by
  the network secret, single-use invites, a shared network document.
- Local proxy (SOCKS5 and HTTP on one port) with path selection and the exit role for verified members.
- Per-node domain list and availability checks (own DNS over HTTPS, TCP, TLS, HTTP) shared with the members.
- Node management page (Vue, built into one committed file) with password sign-in and access modes.
- Tray app for macOS, Linux and Windows; settings file `config.json`.
- Packaging scripts for macOS (`MeshTown.app`), Linux, Windows and OpenWrt.
- Release archives with `SHA256SUMS` and a Homebrew tap (`brew install mesh-town/meshtown/meshtown`).
