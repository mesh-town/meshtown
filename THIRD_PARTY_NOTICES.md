# Third-party notices

This project is licensed under AGPL-3.0-only (see `LICENSE`). It includes or derives from the following third-party
material.

## townjump

- Upstream: https://github.com/mesh-town/townjump (a Rust port of Holesail 2.4.4)
- License: AGPL-3.0-only
- Used as a git dependency, pinned by revision in `Cargo.toml` (crates `townjump-dht`, `townjump-dht-peeroxide`,
  `townjump-keys`, `peeroxide-dht`, `libudx`). Its own notices are in that repository.

## peeroxide-dht and libudx (through townjump)

- Upstream: https://github.com/Rightbracket/peeroxide; our modified copy lives in the townjump repository under
  `fork/peeroxide/`.
- License: `MIT OR Apache-2.0` (texts in `fork/peeroxide/LICENSE-MIT` and `fork/peeroxide/LICENSE-APACHE` there).

## EFF Large Wordlist for Passphrases

- Source: https://www.eff.org/dice, © Electronic Frontier Foundation
- License: Creative Commons Attribution 3.0 United States (CC BY 3.0 US), https://creativecommons.org/licenses/by/3.0/us/
- Embedded unchanged as `src/wordlist/eff_large_wordlist.txt`; details are in `src/wordlist/NOTICE.md`.

## Vue 3.5.43

- Upstream: https://github.com/vuejs/core
- License: MIT
- Built into the management page bundle `web/bundle/app.js` (sources in `web/vue`; the Vue runtime is the only runtime
  dependency of the page).

## Rust dependencies

Rust dependencies in `Cargo.lock` are distributed under their own licenses. A full list of dependency licenses for
binary releases is planned after 0.2.0.

## tun2proxy udpgw protocol

- Upstream: https://github.com/tun2proxy/tun2proxy
- License: MIT, Copyright (c) tun2proxy contributors
- The frame layout and flags of the UDP gateway served by the exit (`src/exit/udpgw/`) follow the `udpgw` client and
  server of tun2proxy 0.8.6. The Android build also links the tun2proxy library as a whole.
