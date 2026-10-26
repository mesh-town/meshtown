# Third-party notices

This project is licensed under AGPL-3.0-only (see `LICENSE`). It includes or derives from the following third-party
material.

## townjump

- Upstream: https://github.com/mesh-town/townjump (a Rust port of Holesail 2.4.4)
- License: AGPL-3.0-only
- Used as a git dependency, pinned by revision in `core/Cargo.toml` (crates `townjump-dht`, `townjump-dht-peeroxide`,
  `townjump-keys`, `townjump-pipe`, `townjump-logger`, `peeroxide-dht`, `libudx`). Its own notices are in that repository.

## peeroxide-dht and libudx (through townjump)

- Upstream: https://github.com/Rightbracket/peeroxide; our modified copy lives in the townjump repository under
  `fork/peeroxide/`.
- License: `MIT OR Apache-2.0` (texts in `fork/peeroxide/LICENSE-MIT` and `fork/peeroxide/LICENSE-APACHE` there).

## EFF Large Wordlist for Passphrases

- Source: https://www.eff.org/dice, © Electronic Frontier Foundation
- License: Creative Commons Attribution 3.0 United States (CC BY 3.0 US), https://creativecommons.org/licenses/by/3.0/us/
- Embedded unchanged as `core/members/wordlist/eff_large_wordlist.txt`; details are in `core/members/wordlist/NOTICE.md`.

## Vue 3.5.43

- Upstream: https://github.com/vuejs/core
- License: MIT
- Built into the management page bundle `core/web_gui/ui/bundle/app.js` (sources in `core/web_gui/ui/vue`; the Vue runtime is the only runtime
  dependency of the page).

## ZXing core 3.5.4

- Upstream: https://github.com/zxing/zxing
- License: Apache-2.0
- Built into the Android app (decoding a photo of a QR code).

## Rust dependencies

Rust dependencies in `Cargo.lock` are distributed under their own licenses, listed in each package's metadata
(`cargo metadata`). Most are `MIT`, `Apache-2.0` or both. Dependencies under other licenses:

| License | Crates |
|---|---|
| `MPL-2.0` | `option-ext` |
| `CDLA-Permissive-2.0` | `webpki-roots` |
| `Unicode-3.0` | the `icu_*` crates, `litemap`, `potential_utf`, `tinystr`, `writeable`, `yoke`, `yoke-derive`, `zerofrom`, `zerofrom-derive`, `zerotrie`, `zerovec`, `zerovec-derive` |
| `Apache-2.0 AND ISC` | `ring` |
| `ISC` | `libloading`, `rustls-webpki`, `untrusted` |
| `BSD-3-Clause` | `curve25519-dalek`, `ed25519-dalek`, `subtle` |
| `BSD-2-Clause` | `spez` |
| `Zlib` | `zlib-rs` |
| `Apache-2.0 AND MIT` | `dpi` |
| `Apache-2.0 WITH LLVM-exception` | `target-lexicon` |
| `AGPL-3.0-only` | `townjump-dht`, `townjump-dht-peeroxide`, `townjump-keys`, `townjump-pipe`, `townjump-logger` (see "townjump" above) |
| `Unlicense` | `ksni` |
| `MIT AND BSD-3-Clause` | `matchit` |
| `(MIT OR Apache-2.0) AND Unicode-3.0` | `unicode-ident` |

## tun2proxy udpgw protocol

- Upstream: https://github.com/tun2proxy/tun2proxy
- License: MIT, Copyright (c) tun2proxy contributors
- The frame layout and flags of the UDP gateway served by the exit (`core/exit/udpgw/`) follow the `udpgw` client and
  server of tun2proxy 0.8.6. The Android build also links the tun2proxy library as a whole.
