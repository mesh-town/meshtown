# meshtown architecture

## What this is

A node with a given network key finds other nodes with the same
key through the public mainline DHT, checks with a secret that they know the same key
(handshake), keeps a QUIC connection with them (iroh, direct or through the n0 relay) and
takes part with them in a shared gossip topic (iroh-gossip). Once per interval a volume probe
(16 KiB / 256 KiB / 1 MiB) runs over the connection to find out whether the transport passes
through the network it runs in.

## Module map

Paths are from the repository root; a path ending in `/` is a directory.

| Path | What the program does with it |
|---|---|
| `core/node/output.rs` | single formatted output to stdout |
| `core/members/network_key.rs` | network key from a label and a random part (normalization, SHA-256), machine form mesh1-…, derived subkeys |
| `core/members/words.rs` | EFF word list, word pool, the random part of the key and its validation |
| `core/transport/net_mainline/record.rs` | format and encryption of the DHT record (member list) |
| `core/transport/net_hyperdht/` | joining the DHT, publishing and reading the record |
| `core/proxy/domains/` | the node's own domain list (a second iroh-docs document without reconciliation), the network's shared document and console commands |
| `core/members/handshake.rs` | handshake with the network secret |
| `core/members/membership.rs` | member bookkeeping and gossip topic events |
| `core/transport/transfer_probe.rs` | volume probe over an established connection |
| `core/node/peer_path.rs` | connection path of a peer and watching for its changes |
| `core/site_checks/http_min.rs` | minimal HTTP/1.1 client over a ready stream and shared TLS settings for checks |
| `core/site_checks/doh/` | the checks' own resolver: DNS over HTTPS and a separate query to the ISP's resolver |
| `core/site_checks/reachability/` | step-by-step check of domain reachability and classification of the result |
| `core/site_checks/checks/` | check records in the shared document, check schedule |
| `core/config/config_file/` | settings file: reading, writing, merging with flags, node secret |
| `core/invite/` | invite string: building, parsing, issuing and redeeming |
| `core/cli/console.rs` | node console: commands from standard input, the first is `invite` |
| `core/node/` | assembling the node from all modules, the main loop |
| `core/cli/` | command-line argument parsing (`keygen`, `run`), including the `--invite` flag |
| `core/proxy/socks5/` | parsing and building SOCKS5 frames, client for tests and the `get` command |
| `core/proxy/route.rs` | choosing a path by domain from `chk/` records and the member list |
| `core/exit/` | exit protocol `meshtown/exit/1`: both sides and byte splicing |
| `core/ports_sharing/` | publishing own TCP ports and opening others': `meshtown/port/1`, `port/tcp/` records |
| `core/proxy/` | SOCKS5 server, connection over the chosen path, console commands |
| `core/web_gui/` | the node's web page and its HTTP API |
| `platforms/<os>/` | the desktop app of one operating system: tray, packaging (see "Crates") |

## Crates

The Cargo workspace (root `Cargo.toml`) has four members:

- `core/` — package `meshtown-core`: library (`core/lib.rs`) and the `meshtown` binary (`core/cli/main.rs`). It is the
  node itself; the binary has no tray icon of its own (the tray backend lives in `platforms/<os>/gui/`).
- `platforms/linux/`, `platforms/macos/`, `platforms/windows/` — packages `meshtown-linux`, `meshtown-macos`,
  `meshtown-windows`. Each is a library (`gui/lib.rs`, the tray) plus the desktop binary (`cli/main.rs`) of the same
  name as the package. Each depends on `meshtown-core` with the `gui` feature, under `cfg(target_os = …)`.

`default-members = ["core"]`: `cargo build` without `-p` builds the `meshtown` node binary, which has no tray icon.
The core's `gui` feature (tray support code, `gui = ["dep:tray-icon"]`) is on by default (`default = ["gui", "net-hyper"]`);
`--no-default-features --features net-hyper` builds the core without it. A platform app is built with `-p meshtown-<os>`.
Crate roots are set by `path` in the manifests, not by directory convention.

Android is a Gradle application (`platforms/android/gui/`). It is not a workspace member: the node is built as a
shared library for it by `platforms/android/build/build.sh`.

## Core and platform boundary

- The core does not depend on anything in `platforms/`: no `path` dependency into that tree, and no `include_*!` or
  `#[path]` in `core/` that leaves `core/`.
- A platform crate depends only on `meshtown-core` among the workspace packages, through its public API (third-party
  crates are fine).
- Scripts and `Makefile`s of a platform stay inside its directory. The exceptions are the path dependency on `core/`
  (only in the platform's `Cargo.toml`), `release/lib.sh`, `release/dist/`, and the workspace root itself, from which
  `cargo` is run (root `Cargo.toml` and `Cargo.lock`, `.tmp/`, `target/`, `dist/`).
- Operating-system code is allowed in the core when a node without a tray needs it (console, file limits, signals).
  Tray, windows and packaging belong to the platform.

`make boundary` (`scripts/check_boundary.py`) checks these rules from the manifests (`cargo metadata`) and from the
text of files: in `core/` only `*.rs` and the manifest, in `platforms/` every text file (sources, `Makefile`s, scripts,
`.md`). It is part of `make check`
and runs in CI. A path that leaves the platform directory for any other place is reported with file and line.

## How the node works (`node::start_node`)

1. Takes the SOCKS5 port on `127.0.0.1`; if the port is busy, it does not start.
2. The single output timer starts by itself on the first log line (`output::JOURNAL`, a lazy static).
3. The node key (`SecretKey`) is taken from the settings file; if there is no file or it is unusable, the key is new
   for this run, and the node writes to the log that the identity is temporary and why.
4. A `MemoryLookup` (memory for found member addresses) and an iroh `Endpoint` are built
   (the n0 preset, but n0's own lookup services are disabled and replaced with `MemoryLookup`;
   the n0 relay stays; port mapping is off).
5. `[node]` lines are printed: version and network, mode (direct+relay / relay only / no relay),
   the list of UDP addresses the node listens on, the node name and the identity state.
6. `iroh-gossip` starts (`Gossip::spawn`) — it adds its own address lookup service.
7. The member context is built (`MemberContext`: handshake secret, member counter,
   event channel, volume probe settings and the node's own name — it goes to the peer in the handshake) along with the protocol router
   (`Router`), which accepts the
   handshake ALPN, the gossip ALPN and `meshtown/exit/1`.
8. A line is printed with the number of address lookup services (n0 is off, but `MemoryLookup` + the gossip
   service + the in-memory lookup of iroh-docs make 3).
9. The node subscribes to the gossip topic (the topic identifier is derived from the network key) and starts
   topic event handling in the background.
10. If the relay is allowed, the relay state report and the network report (NAT) start in the background.
11. If the list of DHT entry addresses is not empty, the `dht::run` loop starts in the background:
    joining the public DHT, periodic publishing of its own record (merged with the previous one) and
    reporting new members through a channel.
12. The node's main loop starts (`run_node_loop`): for each found address — dialing a
    connection and the handshake as initiator; for each `Up`/`Down` event from `membership` —
    printing a member line and inviting it to the gossip topic; every 30 s — a broadcast
    message to the topic if there is at least one member; every 60 s — a `[status]` line with the number of
    members, gossip peers and process memory (RSS from `/proc/self/status`).
13. Brings up the SOCKS5 server and the exit protocol; on the first `run_cycles` round it starts the console
    (`console::spawn`): commands `invite`, checks, proxy, domains.

Stopping: one signal task per process (`core/node/host/signal.rs`) waits for Ctrl-C or SIGTERM, prints a line
about it and stops every network slot of the host; each node closes its router (up to 10 s) and prints "stopped".

The process host (`core/node/host/`) keeps a map of network slots: each slot holds the node controller, the page's
config state and the task running the node cycle. The page and the signal reach the nodes only through that map.

Shared records (site check results, port publications, membership votes): a replica of signed records lives in the
node (`sync::replica`). The replica identifier and its topic are derived from the network key. Members
reconcile the replica over the `meshtown/replica/1` service after the handshake, and topic membership spreads by
flooding over the `meshtown/sync/1` service. The author of a record is the node key, and every entry carries the
author's signature. On a new member the node starts reconciling with it.

## Network key from a label and a random part

A network key for a human looks like this: `label--word-word-group`, for example
`our-gaming-room--finch-unwary-k7m2q9x4rb8vhn3tz01g5`. The string is split at the **first**
`--`: to the left is the label, to the right the random part. The label is not part of the key output —
it is a caption for a human, to tell their networks apart; a typo in the label does not lead to another
network. The random part can also be entered without a label.

Before parsing, both the label and the random part go through the common normalization: Unicode NFKC →
lowercase → spaces and `_` replaced with a hyphen → everything except letters, digits and hyphens
is dropped → consecutive hyphens collapse into one → hyphens at the edges are trimmed.
Normalization is idempotent: calling it again changes nothing. A caveat: NFKC first
**expands** some characters and only then does the filter work — for example, `№` gives
`No`, and `½` gives `1⁄2`, which is `12` after the filter.

The random part consists of three parts joined by hyphens: two different words from a pool of 7,772
words of the EFF list (the list itself has 7,776 words; four words with a hyphen inside are excluded from
the pool — otherwise joining with hyphens would be ambiguous) and a group of 21 characters of the
Crockford base32 alphabet `0123456789abcdefghjkmnpqrstvwxyz` — digits and letters except `i`, `l`, `o`
and `u`. The strength of the random part is 130.8 bits: two different words out of 7,772 give
log2(7772 × 7771) = 25.848 bits, 21 characters of a 32-character alphabet — exactly 105 bits.

When reading the random part, the letters `o` and `O` are treated as the digit `0`, and `i`, `I`, `l`, `L` as
the digit `1` (this is why Crockford base32 was chosen over the RFC 4648 alphabet
`a-z2-7`: in Crockford these letters are excluded from the alphabet precisely because they
are confused with digits, and the mapping on reading finishes the job). So a key copied
by a person from a screen with a letter mistaken for a digit opens the **same** network, not a
neighboring one.

The key is derived from the random part with a fast hash:
```
key = SHA-256( CTX_TAIL_SALT ‖ 0x0A ‖ normalized_random_part )   // 32 bytes
CTX_TAIL_SALT = "meshtown network key tail v1"
```
Argon2id is not used at all: the program sets the entropy, not a human, it is known
exactly and equals 130.8 bits — there is nothing to brute-force, and a slow key derivation function is not
needed here. Pinned by the known-answer test `key_from_tail_known_answer`.

Key derivation takes a few milliseconds. Almost all of
that time is the one-off building of the word list in memory (a `Vec` and a `HashSet` of 7,772 entries, 1.2 ms by
the difference between `keygen` and `keygen --raw`); SHA-256 itself takes under a microsecond. The memory peak during
key derivation is gone: the word list already sits in the binary, and the 64 MiB buffer no longer exists.

The machine form `mesh1-…` (32 random bytes in base32) remains the second valid input
and is printed by the command `meshtown keygen --raw`.

## Protocols

- **Network key** (`core/members/network_key.rs`): the human form is `label--word-word-group` (the label does not
  affect the key), the machine form is `mesh1-` + 52 lowercase base32 characters (32 bytes
  of secret); all other keys are subkeys via `blake3::derive_key` with different contexts.
- **DHT record** (`core/transport/net_mainline/record.rs`, `core/transport/net_hyperdht/`): a mutable BEP44 record, the signing key and salt come
  from subkeys; the value is `nonce(12) ‖ ChaCha20-Poly1305(postcard(Roster))`, ≤ 990 bytes;
  a member record lives 15 min; publishing uses CAS on the previous `seq` (also when the
  previous record cannot be decrypted).
- **Handshake** (`core/members/handshake.rs`): one bi stream, everything under a 10 s timeout, node ids from TLS
  (`remote_id()`/`ctx.me`):
  1. A→B: `nonce_a` (32 bytes).
  2. B→A: `nonce_b` (32) ‖ `responder_tag` (32).
  3. A checks the tag; if it does not match → `close(401, "bad secret")`. Otherwise A→B: `initiator_tag` (32).
  4. B checks; if it does not match → `close(401)`. Otherwise `members.add_verified_conn(A)`, B→A: byte `1`.
  5. A waits for byte `1`, then `members.add_verified_conn(B)`. Timeout → `close(408)`.

After the tag, the initiator sends a name frame (2 bytes of length and the name in UTF-8), and the responder does after the
"accepted" byte. The protocol number is unchanged: an old version neither sends nor reads the tail, so it simply has
no name. The tail is read to the end of the stream, at most 1024 bytes, within the same timeout; a read
error means "no name", not a failure.

### `meshtown/exit/1`

A request to open a connection to a domain. Request frame: version (1 byte, value 1), name
length (1 byte), domain name, port (2 bytes, most significant first). Reply: version (1 byte) and
code (1 byte), then the connection bytes unchanged. Codes: 0 — opened, 1 — the domain is not
in the shared list (the rule is removed, see below; the code stays in the protocol — old exits in the network
still send it, and the requesting side prints its text), 2 — address not obtained, 3 — the connection
did not open, 4 — port not allowed (the rule is also removed, see below; the code stays for the same
reason), 5 — the node does not work as an exit for you, 6 — the request could not be parsed.

Request frame, 4 + N bytes:

| Offset | Length | Field | Value |
|---|---|---|---|
| 0 | 1 | version | 1 (`EXIT_PROTOCOL_VERSION`) |
| 1 | 1 | name length | 1…255 |
| 2 | N | domain name | ASCII, lowercased |
| 2 + N | 2 | port | most significant byte first, any |

Reply, 2 bytes:

| Offset | Length | Field |
|---|---|---|
| 0 | 1 | version, 1 |
| 1 | 1 | code |

The exit checks a request in four steps: permission (whether the requester is a member), frame parsing,
the domain's address, the connection. The check of the domain against the list and of the port is gone: the exit
owner serves any domain on any port and controls only `exit.enabled`; a separate
switch for the exit owner (which domains and ports they are ready to serve) is not part of this
task. There is no personal data in the frame: no client string, no request path — the exit sees
only the domain name and the port.

Path choice on the requesting side (`route::choose_route`, `core/proxy/route.rs`) does not look at the request's domain
at all — one decision for the whole connection at once. The candidates are set by the config field
`route.exits` (node order = priority, the default is an empty list, edited by `PUT /api/config`
on the fly): the list is empty → the node connects by itself; not empty → the candidates are tried in list order —
not the node itself, online and flagged "works as an exit" (`node_state::read_exit_flags`); if the stream to a
candidate did not open (a transport error) → the next one in the list is tried, while any reply
code received is final. No suitable candidates → refusal. The earlier auto-choice by the number of
successful checks of the domain (`route::fallback_peer`) and the "all traffic / list only" switch
are gone from the code. Open connections through an exit that is not in the new `route.exits` list
are closed; direct ones are not.

### `meshtown/port/1`

A request to open a connection to a port published by a node. The publisher replies with a code, then come the
connection bytes (spliced by `exit::splice`); the TCP target is only its own loopback (`127.0.0.1`, then `[::1]`).

Request frame, 3 bytes:

| Offset | Length | Field | Value |
|---|---|---|---|
| 0 | 1 | version | 1 (`PORT_FRAME_VERSION`) |
| 1 | 2 | port | most significant byte first, 1…65535 |

Reply, 2 bytes: version (1) and code. Codes: 0 — connection opened; 1 — the node does not publish this port;
3 — the port does not respond on the node; 5 — the node does not open this port for the requester; 6 — the request could not be
parsed; others (including 2 and 4) are an "unknown code", a refusal for the requester.

The publisher's check order: permission (whether the requester is a member) → frame parsing → its own loopback.

- **Gossip gate** is absent — see "Limitations" below.
- **Threat model** — see the section below.
- **Volume probe** (`core/transport/transfer_probe.rs`): a bi stream, `u32 LE length ‖ zeros`, the reply is the same 4 bytes.

Keys of the domain list document:

| Key | Value | Meaning |
|---|---|---|
| `list/<domain>/` | `v1` | the domain is in the list (author's record) |
| `list/<domain>/` | empty (tombstone, `Hash::EMPTY`) | the author removed the domain |
| `port/tcp/<port>/` | postcard `PortRecord { v, name, audience }`: `v` — 1 byte (1); `name` — varint length and UTF-8; `audience` — varint of the variant (0 = everyone, 1 = list: varint number of keys and 32 bytes each) | the author publishes their port; example "web"/everyone — `01 06 D0 B2 D0 B5 D0 B1 00` |
| `port/tcp/<port>/` | empty (tombstone) | the author withdrew the publication |

## Threat model

An outsider who does not know the network key sees in the DHT only the public signing key, the salt and the
record ciphertext: from them they cannot get the record key, the handshake key, or the
gossip topic identifier (`blake3::derive_key` is irreversible, the contexts of all subkeys are
different). They cannot forge a record without the signing key. They cannot pass the handshake
without the handshake key: the tags are bound to the fresh nonces of both sides and to both
node identifiers from TLS, and replaying an intercepted exchange is useless.

A gossip connection to a node whose `EndpointId` became known to an outsider (for example, from
intercepted traffic) can be opened by them — the gossip ALPN accepts a connection from anyone
who knows the `EndpointId`. But without the network key they do not know the topic identifier and so
can neither receive nor send messages in it. This is a deliberate limitation
(see "Limitations" below, the item about the gate).

A network set by the random part of a key: whoever guessed the random part is a full
member. The key derivation salt is one for the whole application, so an outsider can compute once
the keys for a list of candidates and match them at once against all the records they
see in the public DHT. But there is no need to defend against salt guessing: the random part is issued
by the program, and its 130.8 bits cannot be brute-forced — this is not a human's password, which can be
weak, but a fixed length of random words and characters. That is why a slow key derivation
function (Argon2id) is not used: it would slow down a guessing attack that does not exist anyway, at the cost of
memory and time on every start.

Shared records: anyone who knows the network key can write and read them — write access is shared. The replica
service answers only after the handshake on the network secret, and the replica identifier is derived from the
network key. Any member can remove any domain; the order of edits is decided by the timestamps of the writing nodes.
A record value has no domain, no addresses and no node identifier: the domain is in the record key, and keys are
handed out only to members that pass the handshake. A check record that reached an outsider would say only "some
domain at some node in such-and-such class".

The reachability check must go around the node's own TUN, or it will measure itself.
For now this is ensured by never calling the system resolver anywhere: the DoH server is set
by address, the domain's address is taken from its reply, and a reply from the fake-ip pool (198.18.0.0/15)
is rejected. When TUN appears, the outgoing socket will have to be marked to bypass the tunnel — there is one
place for that: `connect_tcp` in `core/site_checks/reachability/probe.rs`.

## Limitations

- One DHT record holds up to 8 members (after that the oldest are pushed out).
- Relay — only n0 (`RelayMode::Default`), there is no relay of its own.
- The gossip ALPN accepts a connection from anyone who knows the `EndpointId`, but the gossip topic
  is derived from the network key — without the key, topic messages can be neither received nor sent.
- Port mapping on the router (UPnP/PCP/NAT-PMP) is always off
  (`PortmapperConfig::Disabled`) — the node does not touch the router; direct
  paths are measured without port mapping, which is an estimate from below.
- Node names are not unique; an invite is reusable and secret exactly like the network key.
- Without a settings file (`--no-config`), the domain list lives in memory and is lost when the whole
  network restarts.
- The author of records is the node key: with a key from the settings file the author is the same across runs, with
  `--no-config` it is new on every run, and records of past runs stay under the old
  authors.
- There are no limits on the number of domains and records.
- Deleted domains stay in the document as empty records (tombstones) and are not cleaned up.
- Out of scope: domain reachability checks (`chk/`), SOCKS, TUN, own relay.
- The DNS query identifier is zero; for the ISP's reply this is acceptable, because
  addresses from it are not used.
