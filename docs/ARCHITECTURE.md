# meshtown architecture

## What this is

A node with a network key works in five steps:

1. **Discovery.** It finds other nodes with the same key through a record in the public DHT (HyperDHT).
2. **Handshake.** It checks with a secret that the peer knows the same key.
3. **Transport.** It keeps a connection over the `hyper` transport, an adapter to the townjump library, which does
   NAT hole punching.
4. **Relay fallback.** When a direct connection does not come up, the dial falls back to a relay run by another
   member of the same network (see "Relay").
5. **Shared state.** The nodes share a topic (membership by flooding) and a replica of signed records (domain list,
   check results, port publications).

Once per interval a volume probe (16 KiB / 256 KiB / 1 MiB) runs over a connection to find out whether the
transport passes through the network it runs in.

### Relay

A relay pairs only members of its own network. The receiving side sees that the session is relayed (not by whom)
and reports `relay`.

- A member serves as a relay unless it is behind a NAT that gives each connection a different external port. The
  NAT type is read from the transport library's reachability report.
- The member advertises its relay key as the record `relay/v1` in the network's replica. A member with a public
  address writes 38 bytes, the key followed by the address. A member behind a NAT with stable ports writes the bare
  32-byte key and is dialled by key through the DHT with hole punching. Older readers ignore the tail.
- Candidates with an address are tried first. A relay that is unfit is closed and tried again after 600 s.
- A public member that later moves behind a NAT keeps its old address in the record until restart.
- The setting `net.relay` is `network` by default; `off` disables both calling and serving.

The transport sits behind a small interface (`core/transport/net/`): a node identifier, a peer address with hints,
services identified by a label on a stream, and discovery sources. An in-memory implementation
(`core/transport/net/mem/`) backs the tests.

## Services

Each service is a stream label (`core/transport/net/service.rs`):

| Service | Purpose |
|---|---|
| `meshtown/auth/1` | handshake on the network secret (open to everyone) |
| `meshtown/invite/1` | issuing and redeeming an invite (open to everyone) |
| `meshtown/sync/1` | topic membership by flooding (members only) |
| `meshtown/replica/1` | reconciling the record replica (members only) |
| `meshtown/exit/1` | exit protocol: opening a connection to a domain (members only) |
| `meshtown/port/1` | opening a connection to a published TCP port (members only) |

"Members only" means the peer has a live connection that passed `meshtown/auth/1`
(`core/node/start/transports_hyper.rs`).

## Module map

Paths are from the repository root; a path ending in `/` is a directory.

| Path | What the program does with it |
|---|---|
| `core/transport/net/` | transport interface, services, stack facade, discovery multiplexer, in-memory implementation |
| `core/transport/net_hyper/` | the `hyper` transport on townjump (feature `net-hyper`) |
| `core/transport/net_hyperdht/` | the `hyperdht` discovery source: joining the DHT, publishing and reading the record |
| `core/transport/net_mainline/record.rs` | record format and encryption (member list); no networking |
| `core/transport/transfer_probe.rs` | volume probe over an established connection |
| `core/transport/peers_cache.rs` | last seen peer address cache |
| `core/members/network_key.rs` | network key from a label and a random part (normalization, SHA-256), derived subkeys; recognizes an older-format key and refuses it |
| `core/members/network_key/managed.rs` | the key that carries the network creator's node id, its text forms |
| `core/members/access/authority.rs` | a node's standing |
| `core/members/access/managed/` | the tally of an administered network: roles, exclusions, network closing |
| `core/members/access/abilities.rs` | what this node may do in its network (flags for the page and the tray) |
| `core/members/words.rs`, `core/members/wordlist/` | EFF word list, word pool, the random part of the key and its validation |
| `core/members/handshake.rs` | handshake with the network secret |
| `core/members/membership.rs` | member bookkeeping and topic events |
| `core/members/access/` | per-member access rules |
| `core/members/sync/flood/` | topic membership by flooding over `meshtown/sync/1` |
| `core/members/sync/replica/` | record replica: entries, signatures, reconciliation over `meshtown/replica/1`, replica file |
| `core/node/` | node assembly from all modules (`start/`), the main loop, process host (`host/`), output |
| `core/node/peer_path.rs` | a peer's connection path and watching for its changes |
| `core/node/output.rs` | single formatted output to stdout |
| `core/proxy/` | SOCKS5 server, connection over the chosen path, console commands, route choice (`route.rs`) |
| `core/proxy/domains.rs`, `core/proxy/domains/` | the domain list in the replica |
| `core/exit/` | exit protocol `meshtown/exit/1`: both sides and byte splicing |
| `core/ports_sharing/` | publishing own TCP ports and opening other members' ports: `meshtown/port/1`, `port/tcp/` records |
| `core/site_checks/` | reachability checks: `http_min.rs` (minimal HTTP/1.1 client), `doh/` (own resolver), `reachability/`, `checks/` (records in the replica, schedule), `geoip/` |
| `core/autostart/` | start at login: the entry that launches the tray app, registered by the platform crate |
| `core/http_url/` | small URL front end: relay address normalization and the host of a typed domain |
| `core/iface_bind/` | binding outgoing sockets to a named network interface |
| `core/config/config_file/` | settings file: reading, writing, merging with flags, node secret |
| `core/invite/` | invite string: building, parsing, issuing and redeeming |
| `core/cli/` | command-line parsing and subcommands (`run`, `join`, `network`, `member`, …), node console |
| `core/web_gui/` | the node's web page (`ui/`, Vue) and its HTTP API |
| `core/gui/` | tray support code used by the platform crates (feature `gui`) |
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
  (only in the platform's `Cargo.toml`), `scripts/release/lib.sh`, `scripts/release/dist/`, and the workspace root itself, from which
  `cargo` is run (root `Cargo.toml` and `Cargo.lock`, `.tmp/`, `target/`, `dist/`).
- Operating-system code is allowed in the core when a node without a tray needs it (console, file limits, signals).
  Tray, windows and packaging belong to the platform.

`make boundary` (`scripts/checks/check_boundary.py`) checks these rules from the manifests (`cargo metadata`) and from the
text of files: in `core/` only `*.rs` and the manifest, in `platforms/` every text file (sources, `Makefile`s, scripts,
`.md`). It is part of `make check` and runs in CI. A path that leaves the platform directory for any other place is
reported with file and line.

## How the node works (`node::start_node`)

`start_node` (`core/node/start/`) runs four phases and returns a `RunningNode`:

1. **boot**
   - When the proxy is enabled, takes the SOCKS5 port on the configured address (a busy port stops the start).
   - Reads the node secret from the settings file. Without a file, or if it is unusable, the key is new for this run
     and the log says that the identity is temporary and why.
   - Brings up the transports and picks the node name.
   - Prints the `[node]` lines: version and network, mode, listen addresses, name and identity state.
2. **network** — builds:
   - the sync pieces (topic membership and the record replica);
   - the network stack with the service handlers (see "Services");
   - the member context: handshake secret, member counter, event channel, volume probe settings, and the node's own
     name that goes to the peer in the handshake;
   - the discovery sources: `invite`, `cache` (last seen addresses) and `hyperdht` (the DHT record).

   Addresses found by any source go to the main loop through one channel.
3. **serving** — the domain list, the checks, the path choice and the SOCKS5 server.
4. **entry** — subscribes to the topic (its identifier is derived from the network key) and starts the main loop.

A phase error closes the network stack and is returned as is.

The `hyperdht` source periodically publishes the node's own key into a shared roster record and reads the others'
(see "Protocols"); the other nodes' addresses are found through the transport once their keys are known.

The main loop (`run_node_loop`, `core/node/event_loop.rs`):

- For each found address it dials a connection and runs the handshake as the initiator.
- For each `Up`/`Down` member event it prints a member line and updates the topic.
- Every 30 s it broadcasts a message to the topic.
- Every 60 s it prints a `[status]` line (members, topic neighbors, domains, checks, process memory).
- A slower timer prunes stale peers.

Stopping: one signal task per process (`core/node/host/signal.rs`) waits for Ctrl-C or SIGTERM, prints a line
about it and stops every network slot of the host; each node closes its network stack and prints "stopped".

The process host (`core/node/host/`) keeps a map of network slots. Each slot holds the node controller, the page's
config state and the task running the node cycle. The page and the signal reach the nodes only through that map.

Shared records (site check results, port publications, the domain list) live in a replica of signed entries
(`core/members/sync/replica/`).

- The replica identifier and the topic are derived from the network key.
- Members reconcile the replica over `meshtown/replica/1` after the handshake. On a new member the node starts
  reconciling with it.
- Topic membership spreads by flooding over `meshtown/sync/1`.
- An entry's author is the node key, and every entry carries the author's signature.
- With a settings file the replica is stored in the `replica/` directory next to `docs-store/`: `domains.bin`, or
  `<name>-<10 hex>.bin` for additional networks (`core/members/sync/replica/file.rs`). Without a file it lives in
  memory.

## Network key from a label and a random part

A network key for a human looks like this: `label--word-word-group`, for example
`our-gaming-room--finch-unwary-k7m2q9x4rb8vhn3tz01g5`. The string is split at the **first**
`--`: to the left is the label, to the right the random part. The label is not part of the key output —
it is a caption for a human, to tell their networks apart; a typo in the label does not lead to another
network. The random part can also be entered without a label.

Before parsing, both the label and the random part go through the common normalization:

1. Unicode NFKC.
2. Lowercase.
3. Spaces and `_` become a hyphen.
4. Everything except letters, digits and hyphens is dropped.
5. Consecutive hyphens collapse into one.
6. Hyphens at the edges are trimmed.

Normalization is idempotent: calling it again changes nothing. A caveat: NFKC first **expands** some characters and
only then does the filter work. For example, `№` gives `No`, and `½` gives `1⁄2`, which is `12` after the filter.

The random part consists of three parts joined by hyphens:

- Two different words from a pool of 7,772 EFF list words. The list has 7,776 words; four words with a hyphen inside
  are excluded, because joining with hyphens would be ambiguous.
- A group of 21 characters of the Crockford base32 alphabet `0123456789abcdefghjkmnpqrstvwxyz`: digits and letters
  except `i`, `l`, `o` and `u`.

The random part has 130.8 bits of strength. Two different words out of 7,772 give log2(7772 × 7771) = 25.848 bits;
21 characters of a 32-character alphabet give exactly 105 bits.

When reading the random part, the letters `o` and `O` are treated as the digit `0`, and `i`, `I`, `l`, `L` as the
digit `1`. So a key copied from a screen with a letter mistaken for a digit opens the **same** network, not a
neighboring one.

This is why Crockford base32 was chosen over the RFC 4648 alphabet `a-z2-7`: Crockford excludes these letters
because they are confused with digits, and the mapping on reading finishes the job.

The key is derived from the random part with a fast hash:
```
key = SHA-256( CTX_TAIL_SALT ‖ 0x0A ‖ normalized_random_part )   // 32 bytes
CTX_TAIL_SALT = "meshtown network key tail v1"
```
Argon2id is not used. The program sets the entropy, not a human, and it is known exactly: 130.8 bits. There is
nothing to brute-force, so a slow key derivation function is not needed. Pinned by the known-answer test `key_from_tail_known_answer`.

The machine form `mesh2-…` is the second valid input. It is printed by `meshtown network create --key-only --raw`
and `meshtown network key --raw`.

A key in the older format (`mesh1-…`, or three words with a group) is not accepted. The parser recognizes it and
answers that the key comes from an older version and that an administrator has to send a new invite. An older-format
invite is refused the same way.

## Network administration

Every network is administered. Its key is derived from a random base and the creator's node id (`root`), so another
creator gives another network. The human form is `label--word-word-group-<root>`, the machine form `mesh2-…`. An
invite is a version 4 invite: it carries the root next to the label and the addresses (`core/invite/parse.rs`).

The creator is the root and at first the only administrator. The page advises making a second member an
administrator, so that the network can still be managed if the creator's computer is lost.

- The admitted closure starts at the root.
- Any administrator's admission record counts (except a witness's).
- One administrator decides on removing a node.
- The root stays an administrator for good.

The roles live in the network's signed record as records of power (`Grant`, `Revoke`, `Close`) next to the
admissions.

- The tally (`core/members/access/managed/`) replays them in order. A record counts when its author is an
  administrator at that moment, and an exclusion voids the earlier records of the excluded node.
- The last administrator cannot step down (`last_admin`).
- `Close` is written by an administrator and ends the network for everybody: members that see it leave by
  themselves.
- From the published verdicts `access::abilities` derives the flags `can_invite`, `can_remove`, `can_set_admins`,
  `can_delete_network`, `can_leave`. The page and the tray show no control whose flag is off.
- The actions check the same rules again (`access::actions`), so a stale page gets 409 and not an effect.

## Protocols

- **Network key** (`core/members/network_key.rs`): the human form is `label--word-word-group-<root>` (the label does not
  affect the key), the machine form is `mesh2-…`, both with the creator's node id. All other keys are subkeys via `blake3::derive_key` with different contexts.
- **DHT record** (format in `core/transport/net_mainline/record.rs`, publishing and reading in
  `core/transport/net_hyperdht/`): a mutable record whose signing key and salt come from subkeys; the value is
  `nonce(12) ‖ ChaCha20-Poly1305(postcard(Roster))`, ≤ 990 bytes. The roster holds node keys (up to 16 entries).
  An entry lives 15 min. A new `seq` is always greater than the previous one.
- **Handshake** (`core/members/handshake.rs`): one bi stream, everything under a 10 s timeout, node ids from TLS
  (`remote_id()`/`ctx.me`):
  1. A→B: `nonce_a` (32 bytes).
  2. B→A: `nonce_b` (32) ‖ `responder_tag` (32).
  3. A checks the tag; if it does not match → `close(401, "bad secret")`. Otherwise A→B: `initiator_tag` (32).
  4. B checks; if it does not match → `close(401)`. Otherwise `members.add_verified_conn(A)`, B→A: byte `1`.
  5. A waits for byte `1`, then `members.add_verified_conn(B)`. Timeout → `close(408)`.

After the tag, the initiator sends a name frame (2 bytes of length and the name in UTF-8). The responder sends its
frame after the "accepted" byte.

The protocol number is unchanged. An old version neither sends nor reads the tail, so it has no name. The tail is
read to the end of the stream, at most 1024 bytes, within the same timeout. A read error means "no name", not a
failure.

### `meshtown/exit/1`

A request to open a connection to a domain. Request frame: version (1 byte, value 1), name length (1 byte), domain
name, port (2 bytes, most significant first). Reply: version (1 byte) and code (1 byte), then the connection bytes
unchanged.

Codes:

- 0 — opened.
- 1 — the domain is not in the shared list (the exit never sends it; the requesting side still prints its text).
- 2 — address not obtained.
- 3 — the connection did not open.
- 4 — port not allowed (the exit never sends it; the requesting side prints its text).
- 5 — the node does not work as an exit for you.
- 6 — the request could not be parsed.
- 7 — the address is closed to exit (private network, loopback, link-local, multicast, service, or the member's own
  address; `EXIT_CODE_ADDRESS_FORBIDDEN`, `core/exit/frame.rs`).
- 8 — the interface the exit is bound to (`exit.interface`) is missing, down or cannot be bound
  (`EXIT_CODE_INTERFACE_DOWN`).

A code is the final answer: the client does not try other exits.

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

Busy reply, 1 byte: the version byte, then the exit closes the stream for writing. The exit has no free stream
slot (`exit.max_streams` or the memory guard). Clients that predate it read an unanswered request and try the next
exit.

The exit checks a request in four steps: permission (whether the requester is a member), frame parsing, the
domain's address, the connection. The exit serves any domain on any port; the exit owner controls only
`exit.enabled`. The frame has no personal data: no client string, no request path. The exit sees only the domain
name and the port.

Path choice on the requesting side (`route::choose_route`, `core/proxy/route.rs`) does not look at the request's
domain: one choice covers the whole connection. The config field `route.exits` sets the candidates (node order =
priority, the default is an empty list, edited on the fly by `PUT /api/config`):

- The list is empty: the node connects by itself.
- The list is not empty: the candidates are tried in list order. A candidate is not the node itself, is online and
  is flagged "works as an exit" (`node_state::read_exit_flags`).
- The stream to a candidate did not open (a transport error): the next one in the list is tried. Any reply code
  received is final.
- No suitable candidates: refusal.

Open connections through an exit that is not in the `route.exits` list are closed; direct ones are not.

### `meshtown/port/1`

A request to open a connection to a port published by a node. The publisher replies with a code, then the
connection bytes follow (spliced by `exit::splice`); the TCP target is only its own loopback (`127.0.0.1`, then `[::1]`).

Request frame, 3 bytes:

| Offset | Length | Field | Value |
|---|---|---|---|
| 0 | 1 | version | 1 (`PORT_FRAME_VERSION`) |
| 1 | 2 | port | most significant byte first, 1…65535 |

Reply, 2 bytes: version (1) and code. Codes:

- 0 — connection opened.
- 1 — the port is not published.
- 3 — the port does not respond.
- 5 — access to this port is not allowed for the requester.
- 6 — the request could not be parsed.
- Others (including 2 and 4) are an "unknown code", a refusal for the requester.

The publisher's check order: permission (whether the requester is a member) → frame parsing → its own loopback.

## Other protocols

- **Threat model** — see the section below.
- **Volume probe** (`core/transport/transfer_probe.rs`): a bi stream, `u32 LE length ‖ zeros`, the reply is the same 4 bytes.

### Replica keys

| Key | Value | Meaning |
|---|---|---|
| `list/<domain>/` | `v1` | the domain is in the list (author's record) |
| `list/<domain>/` | empty (tombstone) | the author removed the domain |
| `port/tcp/<port>/` | postcard `PortRecord { v, name, audience }`: `v` — 1 byte (1); `name` — varint length and UTF-8; `audience` — varint of the variant (0 = everyone, 1 = list: varint number of keys and 32 bytes each) | the author publishes their port; example "web"/everyone — `01 03 77 65 62 00` |
| `port/tcp/<port>/` | empty (tombstone) | the author withdrew the publication |

## Threat model

### Outsider without the key

An outsider who does not know the network key sees in the DHT only the public signing key, the salt and the record
ciphertext. From them they cannot get the record key, the handshake key, or the topic identifier
(`blake3::derive_key` is irreversible, and the contexts of all subkeys differ).

- They cannot forge a record without the signing key.
- They cannot pass the handshake without the handshake key. The tags are bound to the fresh nonces of both sides and
  to both node identifiers from TLS, so replaying an intercepted exchange is useless.

### Outsider with a node identifier

An outsider who learned a node's identifier (for example, from intercepted traffic) can open a connection to it.
Only the `auth` and `invite` services answer them; every other service refuses a peer that has not passed the
handshake. Without the network key they cannot pass it and do not know the topic identifier.

### Guessed random part

Whoever guessed the random part finds the network's records and passes the handshake, but is not a member until a
member admits it to the member list.

The key derivation salt is one for the whole application, so an outsider can compute the keys for a list of
candidates once and match them at once against all the records they see in the public DHT. There is no need to
defend against salt guessing. The program issues the random part, and its 130.8 bits cannot be brute-forced: it is
not a human's password, which can be weak, but a fixed length of random words and characters. A slow key derivation
function (Argon2id) would slow down a guessing attack that does not exist anyway, at the cost of memory and time on
every start.

### Shared records

Anyone who knows the network key can write and read the shared records, so write access is shared.

- The replica service answers only after the handshake on the network secret, and the replica identifier is derived
  from the network key.
- Any member can remove any domain; the writing nodes' timestamps decide the order of edits.
- A record value has no domain, no addresses and no node identifier. The domain is in the record key, and keys are
  handed out only to members that pass the handshake.
- A check record that reached an outsider would say only "some domain at some node in such-and-such class".

### Reachability checks

The reachability check must go around the node's own TUN, or it will measure itself. For now the node ensures this
by never calling the system resolver:

- The DoH server is set by address.
- The domain's address is taken from the DoH reply.
- A reply from the fake-ip pool (198.18.0.0/15) is rejected.

When TUN appears, the outgoing socket will have to be marked to bypass the tunnel. There is one place for that:
`connect_tcp` in `core/site_checks/reachability/probe.rs`.

## Limitations

- One DHT record holds up to 16 member keys (after that the oldest are pushed out).
- Relay needs a member that can serve it: any member except one behind a NAT with random port mapping. A relay
  starts shortly after the node starts, once the node knows its NAT type. A network where every node is behind a
  random NAT has no relay.
- Node names are not unique.
- An invite is single-use and valid for a day. Whoever presents it first joins, so send it to one person in a
  private message.
- Without a settings file (`--no-config`), the domain list lives in memory and is lost when the whole network
  restarts.
- The record author is the node key. With a key from the settings file the author is the same across runs; with
  `--no-config` it is new on every run, and records of past runs stay under the old authors.
- There are no limits on the number of domains or records.
- Deleted domains stay in the document as empty records (tombstones) and are not cleaned up.
- Out of scope: TUN, a relay run by a non-member.
- The DNS query identifier is zero. This is acceptable for the ISP's reply, because the node does not use addresses
  from it.
