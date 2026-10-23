# Changelog

All notable changes are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/).

Entries below stop at 0.3.0; later changes are in the git history and on GitHub Releases.

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
