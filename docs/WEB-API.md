# HTTP API contract and config schema for the node

This document describes the HTTP API of the node's web page and the schema of its config — the canonical source;
the contract is edited here directly.
The demo node of the page (`core/examples/web_demo/`, the `make web-check` suite — Playwright, `core/web_gui/e2e/`) and
the frontend (`core/web_gui/ui/vue/src/`) must match this document character for character: field names,
error codes, enum values.

How the `config` document (section 2, `WEB-API-config.md`) is laid out on disk — where the file lives, how it survives a crash of the
process and how config versions migrate — is described separately, in `CONFIG-FILE.md`.
There is one schema for both documents; they must not diverge.

**The file is split into parts (600-line limit); the content was moved verbatim.**
Parts:
- This file — 1. HTTP API contract: 1.1 General rules; 1.2 Static files; 1.3 `GET /api/state`; 1.4 `GET /api/log`; 1.5 `GET /api/log/download`; 1.10 Sign-in: `/api/auth/*`; 1.11 Networks; 1.12 Members.
- `WEB-API-config.md` — 1.6 `GET /api/config`; 1.7 `PUT /api/config`; 1.8 Actions; 1.9 Backup (not done now); 2. Config schema.
- `WEB-API-embedding.md` — 3. How it gets into the binary (3.1 Where the listener sits in the entry point; 3.2 What the web listener takes from the config module).

## 1. HTTP API contract

Written into `WEB-API.md` verbatim as below. The node, the demo node and the frontend
must match it character for character: field names, error codes, enum values.

### 1.1 General rules

- Listen on a literal IPv4 address according to the access mode `web.access` (section 2 in `WEB-API-config.md`):
  `localhost` — `127.0.0.1`, `localnet` (default) — `0.0.0.0`; default port 7777. Not
  `localhost` by name, not `::1`: the page does not listen on IPv6. The mode only affects access to the
  node settings page, not the network or traffic.
- A node can work with several networks at once. The routes of one network exist under two addresses:
  `/api/<route>` is the primary network, `/api/net/{slot}/<route>` is the network with the id `{slot}`
  (`main` for the primary one); both are served by the same handler. An id that names no network is
  404 `unknown_net`. Process routes (the log, sign-in, `actions/stop`, `token/rotate`, `/api/nets`,
  `/api/overview`) have no prefix. Details in 1.11.
- All responses are `Content-Type: application/json; charset=utf-8`, except static files and
  the log download.
- Headers on every response:
  `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`,
  `Content-Security-Policy: default-src 'none'; style-src 'self'; script-src 'self'; connect-src 'self' https://api.github.com; img-src data:; form-action 'none'; base-uri 'none'; frame-ancestors 'none'`.
  No response has an `Access-Control-Allow-Origin` header.
- Checks on every request, in this order:
  1. **Peer address** (who connected). `localhost` admits only `127.0.0.0/8`; `localnet` also
     `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16` and `169.254.0.0/16`. The shared ISP address range
     `100.64.0.0/10` and public addresses are not admitted in any mode. The node closes a rejected
     connection immediately without reading the request; if the mode was narrowed while the connection
     was open, the next request on it gets 403 `bad_peer`.
  2. **`Host`** must be exactly one value — the IPv4 literal of the local address of the socket the
     connection was accepted on, with `:<port>` (or without the port if it is 80): `127.0.0.1:7777`
     when opened from this machine, `192.168.1.5:7777` when opened from the network. Otherwise — 403 `bad_host`.
     Names, including `http://localhost:7777`, are rejected here (DNS rebinding protection), and this is
     the expected behavior. A request with an absolute-form target (`GET http://… HTTP/1.1`) is also `bad_host`.
  3. **`Origin`** — for all methods except `GET` and `HEAD`, it is required and must be exactly
     `http://<Host>` (the same `Host` as in item 2). Otherwise — 403 `bad_origin`. For `GET`/`HEAD`
     a missing `Origin` is allowed (a plain navigation by address), and a present one is checked the same way.
     The session now lives in a cookie, so `Origin` also protects against a request from a foreign page: it is
     required not only on `POST`/`PUT`/`DELETE` but on any state-changing method.
  4. **Credentials.** In order: a valid session cookie (see 1.10), otherwise the persistent token
     — the `X-Meshtown-Token` header or the `?token=` parameter in the address (compared byte by byte in constant
     time). A temporary token does not work here: only `POST /api/auth/token` accepts it.
     If nothing matched — 401 `bad_token`. Credentials are required on **all `/api/*` addresses**,
     except `GET /api/auth/status`, `POST /api/auth/token` and `POST /api/auth/login`; static
     page files do not need them. Why this and not "everywhere": the browser requests `<link href="css/…">` and the tag
     `<script src="bundle/app.js">` by itself, a header cannot be attached to them, and there is no way to append
     `?token=` to the static `index.html`; requiring them on `/` is also impossible — after
     `history.replaceState` the address bar no longer has the token, and a plain refresh of the page with F5
     would give 401 instead of the page. What is served is a constant built into the binary: there is no node
     state, key or token in the static files, so reading them from a foreign page is useless. All
     data and all commands live behind credentials on `/api/*`. Checks 1–3 apply to everything,
     static files included.
- Session cookie: `mesh_<first 10 characters of the node public key>=<id>; Path=/; HttpOnly;
  SameSite=Strict; Max-Age=604800` (7 days from sign-in, not extended), without `Secure` — the page
  is served over HTTP. The name with the node key is needed so that two nodes on one host do not overwrite each other's
  sessions (a cookie does not distinguish ports). The session id is 32 random bytes in the same text form as the
  token; the session table lives in process memory, a node restart signs everyone out.
- **An honest limit.** The page is served over HTTP without TLS: on a local network the password, token and cookie
  travel in clear text, and whoever listens to the network can intercept them. The decision was made
  deliberately (outside the scope of the "sign-in to the page" work); the `localhost` mode does not expose them to the network
  at all. A known limitation (not resolved yet): the port of the page itself, published to the
  network via `ports.published`, reaches the page from the address `127.0.0.1` (the node itself connects to the
  local port) and bypasses the access mode; a network member can then guess the password, sharing the attempt counter
  with the owner on `127.0.0.1`. Do not publish the page port. On a router the `localnet` mode
  listens on all interfaces, and the peer filter accepts private addresses: if the ISP's WAN address is
  private (`10/8`, `172.16/12`), only the WAN firewall keeps strangers out (in
  OpenWrt, incoming connections on WAN are dropped by default).
- The error body is always `{"error":"<code>","message":"<in English>"}`, plus the fields specified for
  the particular code.

The full list of codes:

| HTTP | `error` | When |
|---|---|---|
| 400 | `bad_json` | the body did not parse as JSON |
| 400 | `bad_request` | a required request field is missing |
| 400 | `bad_password_format` | the password is shorter than 8 or longer than 256 characters (1.10) |
| 401 | `bad_token` | no credentials (session or persistent token) or the token is wrong; for a token exchange — the token did not match |
| 401 | `bad_password` | the sign-in password did not match (1.10) |
| 403 | `bad_peer` | the peer address is not admitted by the current access mode (1.1, item 1) |
| 403 | `bad_host` | a foreign `Host` or an absolute-form target |
| 403 | `bad_origin` | a foreign or missing `Origin` on a state-changing method |
| 403 | `token_only` | a password reset goes only with the persistent token in the header (1.10) |
| 404 | `not_found` | unknown address |
| 405 | `bad_method` | the method is not supported at this address |
| 409 | `no_password` | sign-in by password, but no password is set or the file is unreadable (1.10) |
| 409 | `no_storage` | nowhere to save the password: the node has no directory for the file (1.10) |
| 409 | `stale_rev` | the config revision is stale — changed in another tab, by the node itself or by hand-editing the file (`rev` lives in the file and survives a node restart); the body has the current `rev` and `config` |
| 409 | `busy` | the previous identical action is still running: a repeated `find-exit`/`check-domain` for the same domain while it has `checking: true`, and a repeated `check-all` while the previous queue has not drained |
| 413 | `too_large` | the body is larger than 256 KiB |
| 422 | `invalid` | the document failed validation; the body has `fields` |
| 422 | `invalid_key` | the network key did not parse; the body has `word` — the word that tripped it, if the error is in a word |
| 422 | `not_available` | enabling something this build does not have yet (autostart, full tunnel) |
| 429 | `too_many_attempts` | too many failed sign-in attempts from this address; the `Retry-After` header is the seconds until the next attempt (1.10) |
| 429 | `rate_limited` | too often; the body has `retry_after_sec`. Reserved for a future rate limit — neither the node nor the demo node returns it now, but the page must understand it as a temporary refusal and not show the "Mesh Town is not responding" banner |
| 503 | `stopping` | the node is already stopping |
| 500 | `config_write_failed` | the settings file was not written (no permission, disk full); nothing is applied, `rev` did not grow — applying a setting without saving it is not allowed: the node would behave differently from what its own file says |
| 500 | `internal` | everything else |

### 1.2 Static files

| Method | Address | Content type |
|---|---|---|
| GET | `/` | `text/html; charset=utf-8` |
| GET | `/css/NN-*.css` (8 files, see `core/web_gui/ui/css/`) | `text/css; charset=utf-8` |
| GET | `/bundle/app.js` (built Vue, committed, `make web-build`) | `text/javascript; charset=utf-8` |

These addresses do not require credentials (rationale in 1.1); the peer address, `Host` and `Origin` are checked.
No directory listing is served, any other path is 404 `not_found`. `/favicon.ico` falls under the same
404; so that the browser does not request it at all and does not litter the console, the `<head>` has
`<link rel="icon" href="data:,">`, and the page replaces it with a ring icon `data:image/svg+xml,…`.
That is why the CSP allows `img-src data:` — exactly for the tab icon; `data:` images do not execute
code, and `default-src 'none'` stays.

### 1.3 `GET /api/state` — state snapshot

Served in full on every request, there are no partial responses. The address `/api/net/{slot}/state`
answers for one network; besides the fields below every state carries `api` (the version of the
multi-network replies, `2`), `slot` (the id of the network the state describes) and `nets` (the brief of
every network of the node, see 1.11). Example (the values are made up, but
all fields are required; `null` is allowed where marked):

```json
{
  "boot_id": "b3f1c2a9",
  "now": 1861.4,
  "config_rev": 7,
  "node": {
    "status": "online",
    "version": "0.5.0",
    "invite_join": null,
    "id": "254b24c7d9",
    "id_full": "254b24c7d9e8...",
    "uptime_sec": 67320,
    "rss_kib": 14648,
    "rss_peak_kib": 16486,
    "web_addr": "127.0.0.1:7777",
    "identity": "persistent",
    "identity_reason": null
  },
  "network": {
    "label": "our-gaming-room",
    "masked": "our-gaming-room--••••••••••••••••••••••••••••••••••",
    "id_hex": "9c1e..."
  },
  "members": { "count": 3, "gossip_neighbors": 2, "voting": 0, "me_excluded": false },
  "socks": { "enabled": true, "listen": "127.0.0.1:1080" },
  "exit": { "enabled": true, "domains": 7, "clients": ["9f3a…"] },
  "transport": {
    "mode": "direct_and_relay",
    "self_path": "direct",
    "relay": { "connected": true, "url": "euw1-1.relay.iroh.network" },
    "nat": "address-restricted cone",
    "listen_udp": ["192.168.1.14:47821", "[::]:47821"],
    "dht": { "joined": true, "nodes": 8, "last_publish_sec": 137 },
    "record": { "published": true, "ttl_sec": 900 },
    "self_addresses": [
      "0652d7aa9d65bf148c912a43439090e6ac2728847144459589ae452497167056@192.168.1.14:47821",
      "0652d7aa9d65bf148c912a43439090e6ac2728847144459589ae452497167056@77.88.21.3:47821",
      "0652d7aa9d65bf148c912a43439090e6ac2728847144459589ae452497167056@https://euw1-1.relay.iroh.network./"
    ]
  },
  "peers": [
    {
      "id": "254b24c7d9", "id_full": "254b24c7d9e8...", "name": "finch-unwary",
      "self": true, "online": true, "path": null, "rtt_ms": null, "exit": true,
      "version": "0.5.0", "checked": 7, "stale": false, "last_seen_sec": 0,
      "through_peer_bps": 0, "through_us_bps": 0
    },
    {
      "id": "9f4a0c71e8", "id_full": "9f4a0c71e8fb...", "name": "red-pond",
      "self": false, "online": true, "path": "relay", "rtt_ms": 186, "exit": true,
      "version": "0.5.0", "checked": 5, "stale": false, "last_seen_sec": 12,
      "through_peer_bps": 1048576, "through_us_bps": 0
    },
    {
      "id": "c03b55d2af", "id_full": "c03b55d2af71...", "name": "willow-candid",
      "self": false, "online": false, "path": null, "rtt_ms": null, "exit": null,
      "version": null, "checked": 7, "stale": true, "last_seen_sec": 254,
      "through_peer_bps": 0, "through_us_bps": 0
    }
  ],
  "domains": [
    {
      "name": "example.org",
      "checks": {
        "254b24c7d9": {
          "verdict": "network_blocked", "reason": "TLS connection reset",
          "dns_ms": 21, "tcp_ms": 34, "tls_ms": 0, "http_ms": 0,
          "status": 0, "bytes": 0, "stalled": false, "age_sec": 142
        },
        "1bd2ec32a5": {
          "verdict": "ok", "reason": "HTTP 200",
          "dns_ms": 18, "tcp_ms": 29, "tls_ms": 41, "http_ms": 248,
          "status": 200, "bytes": 5765, "stalled": false, "age_sec": 96
        },
        "9f4a0c71e8": { "verdict": "ok", "reason": "HTTP 200", "dns_ms": 22, "tcp_ms": 44, "tls_ms": 60, "http_ms": 310, "status": 200, "bytes": 5420, "stalled": false, "age_sec": 300 },
        "c03b55d2af": null
      },
      "checking": false
    }
  ],
  "route": {
    "state": "exit",
    "exits": [
      {
        "id": "1bd2ec32a5", "id_full": "1bd2ec32a56f...", "name": null,
        "online": true, "exit": true, "active": true
      }
    ]
  },
  "ports": {
    "published": [{ "port": 8080, "name": "web", "local": "ok" }],
    "network": [
      {
        "node": "1bd2ec32a56f...", "node_name": null, "online": true,
        "port": 80, "name": "site", "published": true,
        "opened": { "local": 9000, "state": "listening", "error": null }
      }
    ]
  },
  "log": {
    "tags": ["auth","check","checks","console","dht","docs","domains","exit","get","gossip","member","net","node","path","probe","relay","route","socks","status"],
    "ring_lines": 2000,
    "first_seq": 1,
    "last_seq": 4127,
    "tail": [
      { "seq": 4122, "t": 141.3, "tag": "socks", "text": "example.com:443 via 9f4a0c71e8: success, received 18204 bytes, sent 921 bytes, 571 ms" },
      { "seq": 4123, "t": 180.0, "tag": "status", "text": "members 3, gossip peers 2, domains 7, checks 14, memory 14712 KiB" }
    ]
  }
}
```

Exact enum values:

- `node.status`: `"starting"`, `"online"`, `"offline"`, `"stopping"`. `"offline"` also covers a node
  started without a network key and waiting for a network (`network: null`, below).
- `node.identity`: `"persistent"` — the node saved its identity and after a restart will remain the same
  network member; `"temporary"` — it did not save it, and after a restart other members will see this computer as a new member.
  With `"temporary"`, `node.identity_reason` is filled in, otherwise it is `null`. There are three values:

  - `"declined"` — the identity is not saved by explicit instruction (the node was started with `--no-config`);
  - `"file_preserved"` — the file on disk is **intact and deliberately left untouched**: overwriting would have lost
    the person's data, and making a `config.json.bad-<label>` copy failed, so the node preferred to
    leave the original as is. The file may contain the network key;
  - `"file_problem"` — other file troubles. The value is broad and covers a whole set of cases:
    nowhere to write, the file is unreadable or corrupt, no place for the file at all. For a person this is all
    the same — "identity not saved because of the file" — and there is no need to tell them apart by one word.

  **`file_preserved` is separated from `file_problem` not for precision but for what the person should do.**
  For `file_problem` the usual advice is "delete or fix the file". Here that advice is destructive:
  the file is intact, and it may hold the only copy of the network key. So the page shows
  a separate text — the file is not broken, do not delete it — rather than the generic wording about trouble with the file.

  **The page takes the wording of the reason not from this word but from the `file` block** (`message` and
  `field_errors`, section 1.6, `WEB-API-config.md`): the reason is written there in plain language and specifically. `identity_reason`
  answers a single question — whether the person decided this themselves or it is a failure they do not
  know about. The first is reported calmly, the second needs attention.

  The source of the flag is the config layer's `FileReport` (`identity_declined` and `overwrite_refused` next
  to `persistent_identity`, `CONFIG-FILE.md`). The page gets all three values
  as **structured fields, not by parsing the text** of `message`: the text is written for a person and
  changes freely, and a behavior branch must not depend on it.
- `transport.mode`: `"direct_and_relay"`, `"relay_only"`, `"no_relay"`. The display texts are taken
  from `core/node/start/boot.rs`: "direct + relay", "relay only", "no relay".
- `peers[].name` — the node name (`config.node.name`, section 2, `WEB-API-config.md`): its own or one changed by the person,
  including non-Latin scripts. **The field is optional**: the name may be absent or empty —
  then the page shows `id`. The own name is always there, while others' names arrive through the shared network
  document and are matched with peers not instantly, so for the first minutes after joining the network some
  members will have no names; the page must cope with that. The page shows the name in "Members"
  instead of the bare `id`; the tooltip (`title`) with the full `id_full` stays only on the own row,
  as described below.
- `peers[].path`: `"direct"`, `"relay"`, `null` (no path or it is ourselves).
- `peers[].exit`: on the own row `true`/`false` — whether exit is enabled on this node. On other rows
  `true`/`false` — as announced by the node itself (the `node/exit/` key in the shared network document), `null` — the node
  is not in the network or there is no flag (a build without it, the announcement has not arrived yet). The page shows "—"
  where the value is `null`.
- `peers[].through_peer_bps` — integer, bytes/s: our traffic through this peer as an exit (outgoing
  EXIT connections to it; sent + received over a sliding 10 s, divided by 10). Not counted:
  direct exit, port forwarding (`PORT`), volume probes and sync. Always present; the own row and no data — `0`.
  An old node does not send the field — the reader treats its absence as `0`.
- `peers[].through_us_bps` — the same for this peer's traffic through our exit (incoming
  EXIT connections from it). The window, composition and defaults are the same as for `through_peer_bps`.
- `peers[].version` — the Mesh Town program version of the member, a string like `0.1.0` (digits, letters,
  `.`, `-`, `+`, up to 32 characters). The own row is the version of this build, like `node.version`. Other nodes
  announce it with the `node/version/` key in the shared network document at startup, if the recorded one
  differs; `null` — the member has not announced it (a build before this field, the record has not arrived yet) or
  announced an invalid one. It is shown for an offline member too — the last announced. Comparing
  versions is the page's concern.
- `exit.clients` — full ids (`id_full`) of members who are currently going to the internet through this
  node: there is an open exit stream or it closed less than 30 s ago; network members only, in
  ascending order, `[]` — no clients or exit is off. A client that dropped without closing the
  connection (a powered-off node, a network break) disappears not at once but after 32–35 s ;
  the node removes it from network members on connection loss,
  and the list shows only members, so the 30-second QUIC wait
  is not added on top of the window. A client stopped by a signal disappears within a second. A client
  that left and came back sooner than 30 s is visible in the list again until the end of the window, even if
  there is no more traffic.
- `transport.self_path`: `"direct"`, `"relay"`, `null` — how this machine currently reaches the network
  (how the page can show this machine's own path). The values are the same three as for `peers[].path`, so that the page parses them
  with one function.
- `node.version` — the binary version string; shown in the header caption next to the address.
  `node.id_full` — the full node identifier; shown only as a tooltip (`title`)
  on the own row in "Members", it is not in the page text.
- `node.invite_join` — `null` or `{"state": "waiting"|"refused", "message", "label", "expires_unix"}`:
  progress of joining by a single-use invite (`POST /api/network/join` with a `mesh-invite2-…` string).
  `waiting` — the node is looking for members from the string, `refused` — a final refusal with the text `message`;
  it disappears when the node has joined the network, and on a new join, network creation or stop. More in
  `WEB-API-config.md`, section "The `node.invite_join` field and single-use invites".
- `peers[].id_full` — the full identifier of **each** node, not only the own one (unlike
  `node.id_full` above): it is needed as a `route.exits` value when choosing exits on the page and in
  the tray (the checkbox puts `p.id_full` into the list), not only for the tooltip. In the page text it is not shown on
  other rows — only the own row gets a `title`.
- `domains[].checks[<id>].verdict`: `"ok"`, `"dns_mismatch"`, `"network_blocked"`,
  `"site_geoblock"`, `"antibot"`, `"down"` — exactly six values from
  `core/site_checks/reachability/verdict.rs`. `null` instead of an object — there is no record from this member.
  The connection path no longer depends on these checks — there is no `domains[].route` in the snapshot at all.
- `route.state`: `"direct"` (`route.exits` is empty — the node goes out itself), `"exit"` (the list is not empty and
  there is at least one suitable candidate — connections go through the first one in the list), `"unavailable"`
  (the list is not empty, there are no suitable candidates — the node refuses all new connections). The selection
  rule is the same `route::choose_route` function as
  in the proxy.
- `route.exits[]` — the whole `route.exits` list in config order (not only the currently selected
  candidate), with the current status of each node: `id`/`id_full` — identifier, `name` — the same
  value as `peers[].name` for this id (`null` if it has not arrived yet), `online` — whether it is online,
  `exit` — whether its "works as an exit" flag is on (`null` — unknown), `active` — whether this
  node is currently the first suitable candidate (`true` for at most one list element).
- `ports.published[]` — own publications; `local`: `"unknown"` (not checked), `"ok"` (the port responds),
  `"refused"` (does not respond). `ports.network[]` — ports of other nodes: entries with
  the scope "all" and a readable value while the author is in the network (`online` — the same flag as in
  `peers[]`), plus everything opened here (`opened`: `state` `"listening"` or `"bind_failed"`,
  `error` — the error text). `published: false` — the port is open here, but there is no valid record
  for everyone; `node` — the full identifier. Own publications are not in `network`.
- `log.tail` — exactly the last 6 lines, for the "Recent events" card on "Home".
- `boot_id` — random 8 hexadecimal characters, new on every process start.
  A change of `boot_id` means the node restarted: the page clears the log and rereads the config.
- `now` — seconds since node start with one decimal place, the same scale as the log lines
  (`core/node/output.rs:22`).
- `network` — `null` on the very first run, while there is no network yet: the person has not created one and has not joined
  any. Page behavior in this case: on "Home" the panel "You haven't joined a Mesh Town network yet" with the buttons "Join with an invite" and "Create a new network"; the node does not create a network itself.

What each interface element takes from this snapshot is in the table in section 4.

### 1.4 `GET /api/log` — log

Parameters: `after` (integer, optional), `limit` (1…2000, default 500) and `net` (a network id, optional: only the
lines written for that network, process-wide lines are left out; `main` selects the main network). There are no more
parameters — in particular, no `tag`, see below.

Filtering by tag and substring search are **done by the page, not the node**: the whole buffer is already on the page,
and filtering on the node would mean that changing a chip requires downloading the log again and
the `after` cursor stops being continuous. The `tag` parameter is not introduced into the contract at all.

```json
{
  "boot_id": "b3f1c2a9",
  "first_seq": 2128,
  "last_seq": 4127,
  "gap": false,
  "lines": [
    { "seq": 4124, "t": 240.6, "tag": "member", "text": "- c03b55d2af: not responding for 4 min (members: 2)" },
    { "seq": 4125, "t": 241.0, "tag": "gossip", "text": "peer - c03b55d2af" }
  ]
}
```

- `lines` are sorted by ascending `seq`, with no gaps inside the response.
- `gap: true` — the requested `after` is older than `first_seq`, some lines have already been evicted from the ring;
  the page clears the window and draws what arrived.
- `text` is a ready-made string **without** the timestamp and tag: the page assembles them by the format
  `core/node/output.rs:22`, that is `"+" + t.toFixed(1).padStart(7) + "s [" + tag + "] " + text`.
  This way the node does not spend bytes repeating what is already in the fields.
- `net` (string) is present on a line only while more than one network runs and the line belongs to one of them;
  otherwise the line has exactly the four fields above. With several networks the page shows it as `[<net>] ` before `text`.

### 1.5 `GET /api/log/download` — log download

Returns the whole buffer as `text/plain; charset=utf-8` with lines in the exact node format
(`+  141.3s [socks] …`), the header
`Content-Disposition: attachment; filename="meshtown-<boot_id>.log"`. While more than one network runs, the lines that
belong to a network have `[<id>] ` before the text (`+  141.3s [socks] [ab2c3d] …`); other lines are unchanged.
Credentials are accepted as everywhere (1.1); the session cookie goes along by itself on a plain navigation, the persistent token — via the `?token=` parameter, because the download is a plain navigation without headers.


### 1.10 Signing in to the page: `/api/auth/*`

All endpoints pass the 1.1 checks (peer, `Host`, `Origin`); state-changing methods — with `Origin`. Errors are
in the usual form `{"error","message"}`. The `via` field says how the session was opened: `"token"` — the persistent
token (the tray and scripts read it from the `web-token` file), `"temp"` — a temporary token from the node
output, `"password"` — a password; `null` — the request has no credentials.

| Method, path | Credentials | Body | Response | Errors |
|---|---|---|---|---|
| GET `/api/auth/status` | not needed | — | 200 `{"password_set":bool,"access":"localhost"\|"localnet","listening":same,"via":null\|"token"\|"temp"\|"password"}` | — |
| POST `/api/auth/token` | not needed | `{"token":"…"}` | 200 `{"via":"temp"\|"token"}` and `Set-Cookie` | 400 `bad_request`, 401 `bad_token`, 429 `too_many_attempts` with `Retry-After` |
| POST `/api/auth/login` | not needed | `{"password":"…"}` | 200 `{"via":"password"}` and `Set-Cookie` | 400 `bad_request`, 401 `bad_password`, 409 `no_password`, 429 `too_many_attempts` |
| POST `/api/auth/logout` | any | `{}` | 200 `{}` and `Set-Cookie` with `Max-Age=0` | — |
| PUT `/api/auth/password` | session or token | `{"password":"…"}`, 8–256 characters | 200 `{"password_set":true}` | 400 `bad_request`, 400 `bad_password_format`, 409 `no_storage`, 500 `internal` |
| POST `/api/auth/reset` | only `X-Meshtown-Token` | empty | 200 `{"urls":[…],"expires_sec":600}` | 403 `token_only`, 500 `internal` |

What it means:

- **`status`** — the page uses it to decide what to show: the sign-in screen, the first-password screen or
  the shell. `access` is the mode from settings, `listening` is the address the process listens on right now;
  they diverge until the widened mode (`localhost` → `localnet`) takes effect (after a
  node restart): narrowing takes effect at once, widening only after a restart. An unreadable
  password file counts as "no password" (`password_set:false`), sign-in only by token.
- **`token`** — exchanging a token for a cookie. Accepts a temporary token (single-use: spent by the very first
  presentation, lives 10 minutes, no more than 8 alive at once, in process memory, 52 characters) or a
  persistent one. A temporary token works only here, not as a header or `?token=` on other
  addresses. When opening an address with `?token=`, the page calls this endpoint itself and removes the token from the
  address.
- **`login`** — password check; the comparison (PBKDF2-HMAC-SHA256, 600,000 iterations) runs on a background
  thread one at a time, so as not to occupy the node's worker threads. The password is read from the file on
  every sign-in (format — `CONFIG-FILE.md`, section 8.2): `password reset` on a running node
  takes effect at once.
- **`password`** — set or change the password; the old one is not asked for (the session has already proven
  ownership). A change invalidates all other sessions and all live temporary tokens. Length is counted in characters.
- **`reset`** — the same as `meshtown password reset`: deletes the password file, invalidates all sessions and all
  previously issued live temporary tokens, then issues a new one (after the reset only it works) and returns sign-in addresses with it (with `localnet` — two: `127.0.0.1` and
  the address on the local network, if found). Only with the persistent token in the header: a cookie and `?token=`
  do not work. The attempt limiter does not count this endpoint.
- **`logout`** — closes the request's session and erases the cookie; without a session — just erases the cookie.
- `POST /api/token/rotate` (1.8 in `WEB-API-config.md`) invalidates all sessions except the request's own session.

**Attempt limiter** — by peer IP, separately for each address: 5 failures in 15 minutes block the
address for 60 seconds, each next failure after the block ends doubles the period, ceiling 15 minutes.
While an address is blocked, `token` and `login` answer 429 with `Retry-After` (seconds, at least 1), do not
check the password and it does not count as a failure; success resets the counter. A failed token exchange is
also a failure. There is no global counter, otherwise anyone on the network could lock out the owner. The table holds up to 1024 addresses.

**Sessions:** no more than 64 at once (on overflow the oldest is evicted), lifetime 7 days from
sign-in without extension. The persistent token, `X-Meshtown-Token` and `?token=` work for the tray, e2e and scripts
as before. There is no CSRF token: for a state-changing method `Origin` is required and checked against `Host`, the cookie is
`SameSite=Strict; HttpOnly`, a foreign site can neither send the cookie nor forge `Origin`.

### 1.11 Networks: `/api/net/{slot}`, `/api/nets`, `/api/overview`

A network slot is one network the node works with. Its id is six characters `[a-z2-7]`; the primary
network is `main`. A slot may be empty: it waits for a key (see `network/join` and `network/create`).

**Route classes.** The network routes (`state`, `config`, `peer-addresses`, `network/key`,
`network/invite`, `network/join`, `network/create`, `domains`, `actions/find-exit`, `actions/check-domain`,
`actions/check-all`, `actions/restart`, `members` and `members/{exclude,unexclude,admit,unadmit,block,unblock,forget,unforget,leave}`)
exist twice: under `/api` for the primary network and under `/api/net/{slot}` for any network. All other
routes are process routes and exist once, without a prefix. `/api/net/{slot}/log` does not exist (404
`not_found`): the log is one for the process, filter it with `?net=`. The address of a slot that was left or
does not exist is 404 `unknown_net`.

**`NetBrief`** — the short form of one network, used in `nets`:

| Field | Meaning |
|---|---|
| `id` | the slot id |
| `label`, `id_hex` | the network label and its identifier; `null` for an empty or broken slot |
| `status` | the node status of the slot, or `broken` |
| `exit_enabled`, `proxy` | the slot offers an exit; the slot is the one the local proxy uses |
| `route_state`, `exits` | how traffic is routed and the exits in use |
| `members_voting`, `members_admitting` | member counts |
| `joining`, `me_excluded`, `me_pending` | an invite is being redeemed; this node is excluded; its admission waits |
| `peers_online`, `peers_total` | peers online and all known peers |

A broken slot (its files could not be used) has no node: `status` is `broken`, flags are `false`, counters
are 0, `label` and `id_hex` are `null`. Its only action is `POST /api/net/{slot}/members/leave`; every other
address of it is 404 `unknown_net`.

**`GET /api/nets`** — 200 `{"api":2,"nets":[NetBrief…]}`: the primary network first, then the others in
the order the node started them, broken slots last.

**`POST /api/nets`** — adds an empty slot that waits for a key. The body is a JSON object (`{}`). 201
`{"slot":"<id>","nets":[…]}` for a new slot; 200 with the same body when an empty slot already waited
(it is handed back, none is added). 409 `too_many_nets` with `limit` (8) at the limit; 409 `busy` when the
node cannot add networks (it is stopping, or it runs without a settings file).

**`GET /api/overview`** — 200 `{"api":2,"nets":[…],"slots":[…],"restart_slots":[…]}`. Each element of
`slots` is `{"slot":"<id>","state":{…},"members":{…}}` with what `/api/net/{slot}/state` and
`/api/net/{slot}/members` would answer; `members` is `null` where that route answers an error (no network),
and both are `null` for a broken slot. `restart_slots` lists the slots whose state has settings waiting for a
restart.

**Leave network: `POST /api/net/{slot}/members/leave`** (also `/api/members/leave` for the primary
network). The body is `{"confirm":true}`, otherwise 400 `bad_request`. The node first records its own
leaving in the network (if it has one), then the slot is taken out:

- An added slot disappears at once (its address is then 404 `unknown_net`); the node stops it in the
  background and moves its folder aside to `nets/left-<id>-<time>/`. When a neighbour was online, the node
  waits a few seconds before stopping so the record reaches it.
- The primary network stays as an empty slot with a new node identity; its key and the publications are
  dropped from the settings file, its files are moved to `nets/left-main-<time>/`. The page needs no
  restart; after a process restart the cookie of the page is renamed, so sign in again.
- 200 `{"left":bool,"announced":bool,"nets":[…]}` plus the member list fields of the other `members/*`
  replies (`me`, `members`, `counts`, …; with no members and zero counts when the slot had no network or is
  broken). `left` — the record was written (false when the node was
  already excluded or the slot had no network); `announced` — a neighbour was online while the record was
  written. If the node refuses to write the record (409 `busy`, 500 `internal`), the slot is not touched.
- 409 `busy` for the primary network when its key comes from `--key` or `MESHTOWN_KEY`: restart without it.
- A broken slot is only set aside: 200 `{"left":false,"announced":false,"nets":[…]}` plus an empty member list (`"members":[]`, zero counts).

**`POST …/network/join` and `…/network/create`** act on the slot of the address. A slot that already works
with a network refuses a different one: 409 `network_present` (`this tab already has a network: leave it
first, or add a network`), nothing is written and an invite is not spent; `create` always refuses such a
slot, `join` of the slot's own network is accepted as before. A network that another slot already works
with is 409 `net_already_added` with the field `slot` — the id of that slot. `keep_ports` is accepted
and has no effect on a slot without a network.

### 1.12 Members: `GET {base}/members` and the member actions

`{base}` is `/api` for the primary network or `/api/net/{slot}` for any other (section 1.11); the network is chosen
only by the prefix, no body carries a network. An unknown slot is 404 `unknown_net`.

**`GET {base}/members`** — 200, the member list as this node sees it:

| Field | Meaning |
|---|---|
| `me` | the full id of this node |
| `me_excluded` | the members banned this node |
| `me_removed` | the members removed this node from the member list (a removed node is told so when it dials a member) |
| `me_pending`, `me_admission` | this node waits for confirmations; its request while it waits, else `null` |
| `need` | votes the network needs for a decision: 1 while at most two nodes are active, else 2 |
| `counts` | `{"members","voting","excluded","unlisted","pending","forgotten"}`; removed nodes are only in `forgotten` |
| `members` | the rows, see below |
| `changed` | only in replies to actions: whether the action changed anything |

A row of `members`: `id` (short), `id_full` (64 hex digits), `name`, `self`, `online`, `status`, `vote`, `excluded`,
`blocked_by_me`, `denied_by_me`, `forget`, `last_seen_sec`, `last_refusal`, `admission`.

- `status` is one of `member`, `pending`, `unlisted`, `forgotten`, `excluded`; rows are ordered in that way after the
  own row. `forgotten` is a node removed from the member list by enough votes.
- `forget` is `null` or the circle of votes to remove the node: `{"votes","need","proposed_by","voters":[…],"mine",
  "at_unix"}`. `votes` counts the members who voted, `voters` and `proposed_by` are short ids, `mine` — this node has
  voted, `at_unix` — when the node was removed (`null` while the circle waits for more votes).
- `denied_by_me` is `null` or why this node refuses the row: `left`, `excluded`, `voted_by_me`, `removed_by_me` (this
  node voted to remove it, the circle is still open), `blocked`, `removed`, `unlisted`, `refused`.
- `last_seen_sec` is the number of seconds since the last sign of life of the node (a connection, telemetry, the
  address cache or a write in the shared document); 0 for a node connected now, `null` when there is no sign at all.

**Actions.** All are `POST {base}/members/<name>` with `<name>` one of `exclude`, `unexclude`, `admit`, `unadmit`,
`block`, `unblock`, `forget`, `unforget`, `leave`. `exclude`, `unexclude`, `admit`, `unadmit`, `block`, `unblock`,
`forget` and `unforget` take the body `{"id":"<64 hex digits>"}` (`exclude` also an optional `reason`); a missing `id` is 400 `bad_request`, a
malformed one 422 `invalid`. The reply is 200 with the member list after the action, with
`changed`.

- `forget` — this node votes to remove a node that is not connected from the member list. The vote is recorded as a
  signed entry of this node; the node is removed when the votes reach `need` (a second member repeats `forget` for
  the same id). The proposer hides the node at once; it is hidden for the others when the circle completes.
  Repeating the same vote is `changed:false`.
- `unforget` — withdraws this node's vote; when the votes fall below `need` the node is a member again for everyone.
- A new admission of the node (an invitation) made after the votes ends the circle: those votes no longer count.

Refusals are 409 with `error` set to the code: `self_target` (own id), `not_member` (the id is not in the list),
`left` (this node left the network), `not_pending`, `not_confirmed` (`admit`, `unadmit`), `online` (`forget` of a node
that is connected now), `not_forgotten` (`unforget` without a vote of this node). A write failure is 500 `internal`.
