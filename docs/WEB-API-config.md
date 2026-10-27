# HTTP API contract and config schema for the node, continued (2)

Part 2 of 4. A continuation of section "1. HTTP API contract" (subsections 1.6–1.8) and section
"2. Config schema". Part 1 (1.1–1.5) is `WEB-API.md`. Part 3 (section 3) —
`WEB-API-embedding.md`. Part 4 (1.12) is `WEB-API-members.md`.

---

### 1.6 `GET /api/config` — config

For an added network (`/api/net/{slot}/config`, see 1.11 in `WEB-API.md`) the process fields (`exit.max_streams`, `socks.*`,
`web.*`, `log.*`, `node.memory_*`, `autostart.*`, `full_tunnel.*`) show the primary network's values and are
listed in `read_only`; `cli_overrides` lists the
network fields set by launch flags (every flag except `--key` applies to added networks too). A `PUT` leaves them as they are without an error, writes only
the network fields to the slot's file, and answers 409 `busy` while the slot has no network.

```json
{
  "rev": 7,
  "config": { "…": "see section 2" },
  "read_only": [
    "network.label", "network.masked", "web.port", "web.enabled",
    "autostart.available", "autostart.reason",
    "full_tunnel.available", "full_tunnel.reason"
  ],
  "restart_fields": [
    "socks.enabled", "socks.port",
    "checks.doh_server", "checks.provider_dns"
  ],
  "restart_pending": [],
  "cli_overrides": ["socks.port"],
  "file": {
    "path": "/Users/example/Library/Application Support/meshtown/config.json",
    "state": "ok",
    "writable": true,
    "message": null,
    "field_errors": []
  }
}
```

- `read_only` — fields that the node ignores on save. The order in `read_only`, `restart_fields`,
  `changed` and `restart_pending` is the field order in the file (rows of the `config_file::fields::FIELDS` table), the composition is as before. `web.enabled` was added for the same
  reason as the others: the page does not turn itself off, otherwise there would be nothing to turn it back on with.
- `restart_fields` — fields whose change takes effect only after the node restarts.
  By this list the page writes a warning **before** saving, not after. `web.port` is not in this
  list, although it would require a restart: it is in `read_only`, not changed from the page at all,
  and a row in `restart_fields` would be dead. It will appear there when the field becomes editable.
  For the same reason `autostart.enabled` and `full_tunnel.enabled` are not in the list — turning them on
  is rejected with the `not_available` code.
- `restart_pending` — fields that are already saved but not yet applied (the node has not been restarted).
  By them the page keeps a bar "takes effect after the node restarts".
- `cli_overrides` — fields overridden by the flags the node is currently started with (for example,
  `["socks.port"]`). The command line is stronger than the file **per field**, not per document: the other
  fields of the file stay in force.

  **The `config` document always carries the value from the file, not the effective one.** Flags are not
  mixed into it. The reason is that saving goes with the whole document (section 1.7): mix a
  flag value in here, and the very first save of any other setting would silently write the flag into the file
  over whatever was there. The effective (flag-set) value is taken by the page from the state
  snapshot (section 1.3, `WEB-API.md`, for example `socks.listen`) and shown next to the field.

  So a field from `cli_overrides` is edited and saved like a normal one, and the page labels
  under it "set when the node was started — not changed here, will apply after a restart without the flag" and shows what
  is in effect now. Saving is **allowed**: silently accepting an edit that affects nothing on
  this run is not acceptable, but forbidding it is not either — the person configures the node for the future, and
  the saved value goes into the file and works from the next run without the flag.

  One exception — `network.key`, if the node is started with the key flag or an environment variable:
  `network.label` and `network.masked` in the document show the effective (flag-passed) key,
  not the one in the file. This is safe exactly because both fields are in `read_only` — the node ignores them
  on save, and they will not travel back into the file.
- `file` — the state of the settings file on disk (`CONFIG-FILE.md`, section 5.1): `path` —
  where the node writes and reads; `state` — one of `ok`, `created`, `unreadable`, `invalid`;
  `writable` — whether the directory can be written to at all; `message` — the reason in plain language, from the node;
  `field_errors` — fields that did not survive reading the file, of the form
  `[{"path": "socks.port", "message": "file has 80, port is in use; default 1080 used"}]`.
  Page behavior: with `writable: false` (router image, flash drive) the "Save" button is disabled
  **before** pressing; with `state: "unreadable"` or `"invalid"` — a bar "an unreadable
  config is on disk, saving will replace it, a copy will go to `config.json.bad-<date>`"; `field_errors`
  are shown as a bar above the form and disappear after the first successful save.

  **`writable` is computed by the web listener itself, not by the config module.** The latter answers the question "is the
  "read-only" bit set on the file itself" and does not look at the directory at all — yet saving fails also
  when the file can be written but the directory cannot (writing goes through a temporary file next to the target, see
  `CONFIG-FILE.md`). So `writable` in the response is the result of a directory probe
  that the web listener makes on every `GET`/`PUT /api/config` request: the probe is cheap, while the
  implied "the directory does not change" is wrong on removable media and in a router image exactly
  in the cases the field exists for. If the directory probe passed but the "read-only" bit
  is set on the file — `writable: false`: there is still nowhere to write.

### 1.7 `PUT /api/config` — saving the whole config

Body:

```json
{ "rev": 7, "config": { "…": "the whole document" } }
```

Success, 200:

```json
{
  "rev": 8,
  "config": { "…": "the document as the node sees it after applying" },
  "read_only": ["…"],
  "restart_fields": ["…"],
  "restart_pending": ["socks.port"],
  "changed": ["socks.port", "checks.interval_sec"],
  "cli_overrides": ["socks.port"],
  "file": {
    "path": "/Users/example/Library/Application Support/meshtown/config.json",
    "state": "ok",
    "writable": true,
    "message": null,
    "field_errors": []
  }
}
```

- `changed` — which fields the node actually changed (after ignoring `read_only`).
- `restart_pending` — which of the changed ones await a restart. An empty list means everything was applied at once.
- The response always contains the full document: the page redraws the form from it and never
  assumes that its value has "already applied".
- `cli_overrides` and `file` — the same fields with the same meaning as in `GET /api/config` (section 1.6):
  which fields are currently overridden by launch flags, and the state of the file on disk after the write.
  Saving a field from `cli_overrides` is allowed: the value goes into the file and works from the next
  run without the flag, the effective (flag-set) result of this run does not change. In the response
  such a field is returned as **what is now in the file** (just saved), not as the
  effective value — see section 1.6, the document is always about the file.

Stale revision, 409:

```json
{
  "error": "stale_rev",
  "message": "settings were changed in another tab or by Mesh Town itself",
  "rev": 9,
  "config": { "…": "the node's current document" }
}
```

Nothing is saved. The page shows a bar "The settings changed while you were editing" and a button
"Reload (your changes will be lost)": on press the form is redrawn from the sent
document, and the user's edits are lost **explicitly**, with their consent. There is no silent merge —
there is nothing to merge a partly changed document with, without field markup. The cost of a mistake here is not zero
(the person may have filled in the form again), so the button states the consequence, not just the action:
a bare "Reload" does not say what will be lost.

Rejection by content, 422 — **the whole document** is validated at once, **all** errors are returned:

```json
{
  "error": "invalid",
  "message": "settings were not saved",
  "rev": 7,
  "fields": [
    { "path": "socks.port", "message": "port 80 is used by another process" },
    { "path": "checks.interval_sec", "message": "less than 30 s is not allowed" }
  ]
}
```

A `ports.opened` list that adds a `local` already opened in another network of this node is refused with 422 and
the text `local port L is already opened in network "<name>"` (`local UDP port L …` for UDP, counted by `(proto, local)`) (`<name>` is the network's label, or `Network` and the
first four characters of its id when it has none). Only a `local` that is new in the list counts: a conflict already
written in the file does not block saving other fields.

The order of errors in `fields` is the field order in the file. The error text for an editable field is the `put_text`
of the table row, if it is set (for `ports.published`, `ports.opened`),
otherwise the expected value of the form: `node.name` — "a string of at most 64 characters, non-empty after trimming
whitespace", `web.access` — "localhost or
localnet", `autostart.enabled` and `full_tunnel.enabled` with a wrong type — "false (enabling is not available in
this build)". For non-editable paths — the raw file parse message; `version` — "only version 1
is supported", a path without a dot — "an object was expected".

The revision does not grow, the node config does not change in any field. The page colors the fields from `fields` and
writes `message` under them — the texts come from the node and are not composed on the page.

Enabling something unavailable, 422 `not_available`:

```json
{
  "error": "not_available",
  "message": "autostart will appear together with the service",
  "fields": [{ "path": "autostart.enabled", "message": "not available in this build" }]
}
```

### 1.8 Actions

All actions are `POST`, the body is JSON (or empty), the response is JSON. Actions do not touch the config and do not
change `rev`, except those explicitly marked — `POST /api/network/join` and `POST /api/network/create`
write a new network key into the config and raise `rev`.
Network actions also exist under `/api/net/{slot}/…` and act on that network. `network/join` and
`network/create` answer 409 `network_present` when the slot already works with another network, and
`network/join` answers 409 `net_already_added` (with `slot`) when another slot has the network; see 1.11 in
`WEB-API.md`. Joining a Managed network by an invite that was not made by an administrator is refused after the answer: `node.invite_join` becomes
`{"state":"refused","message":…}` (there is no HTTP code for it). `network/inspect`, `network/delete` and the member action `members/admin` also exist under `/api/net/{slot}/…`.

| Address | Body | Response | What it does |
|---|---|---|---|
| `POST /api/actions/find-exit` | `{"domain":"example.org"}` | 202 `{"domain":"example.org","added":true}` | The "Site won't open" scenario from "Home": adds the domain to this node's list and puts it into an out-of-turn check on this node. One address, because it is one user intention. |
| `POST /api/actions/check-domain` | `{"domain":"x.com"}` | 202 `{"domain":"x.com"}` | An out-of-turn check of one domain. In the snapshot the domain gets `checking: true`, then an updated record. The `⟳` button in the domain row (section 4.5). 409 `busy` if the domain already has `checking: true`. |
| `POST /api/actions/check-all` | empty | 202 `{"queued":7}` | The "Check all" button. |
| `POST /api/domains` | `{"domain":"figma.com"}` | 200 `{"domain":"figma.com"}` | Add a domain to this node's list (not passed on to other members). 422 `invalid` — the name did not parse. |
| `DELETE /api/domains?name=figma.com` | — | 200 `{"domain":"figma.com"}` | The cross in the domain row. The name is in the parameter, not in the path: a domain contains dots and would otherwise need escaping. |
| `GET /api/peer-addresses` | — | 200 `{"lines":["0652d7aa9d65bf148c912a43439090e6ac2728847144459589ae452497167056@192.168.1.14:47821", "…"]}` | The same as `transport.self_addresses`, as a separate address — so that the "Copy" button does not depend on the moment of polling. |
| `POST /api/peer-addresses` | `{"text":"9caa709ad9588475e74ad3f896feb1937bf3a5d64b101af7358f6cf2ee799d96@77.88.21.3:47821\n…"}` | 200 `{"added":2,"rejected":[{"line":"junk","message":"no @ separator"}]}` | Manual address exchange under strict NAT. Parsing is line by line, empty lines are skipped, partial success is not an error. |
| `POST /api/network/join` | `{"key":"our-gaming-room--aliens-congested-4dd5mwxvn77e6pptnnw01-glbdthzco34lodbfmhh6knwjvto4c7fqz6irhnyn367juokngaya"}` | 202 `{"label":"our-gaming-room","restart":true}`; for an invite — 202 `{"label":…,"restart":false,"waiting":true}` | Joining someone else's network. Accepted: the human-readable key form, the machine form `mesh2-<base32>` and a single-use invite `mesh-invite2-…` from `GET /api/network/invite`. **A key or `mesh2-`:** the node restarts, the connection to the current network is lost — the page warns before sending; the config changes (the new key goes into the file), `rev` is raised; 422 `invalid_key` with the `word` field if it tripped on a word, without `word` — if the error is in the tail. **An invite:** the string has no network key, only a single-use secret, a label and addresses of up to five members. The response comes at once, 202 with `"restart": false` and `"waiting": true` (the label is `label`, `null` if there is none); there is no restart. Then the node itself looks for members from the string and presents the secret over the `meshtown/invite/1` protocol until it gets the key or gets a final refusal; progress is visible in `/api/state` in the `node.invite_join` field (below). Once it gets the key, the node writes it to the settings file (honoring `keep_ports`) and joins the network without restarting the process; if the file was not written, the node joins anyway and the reason goes to the log. The addresses from the string apply only to this join and are not written to the config. A new `join` call, network creation and node stop cancel the wait. 422 `invalid_key` **without** the `word` field, with `message` as ready text for a person: an old-format string `mesh-invite1-…` ("This invite is of the old format (mesh-invite1-): it contained the network key, and such invites are no longer accepted. Ask a network member for a new invite from the updated program."), an invite of the previous format ("This invite is of an older format: such invites are no longer accepted. Ask a network member for a new invite — from the updated program."), a string older than a day ("This invite was issued more than a day ago and is no longer valid. Ask a network member for a new one."), a corrupted string. A spent invite, one unknown to the members or a foreign one is answered not with 422 but with the `refused` state in `node.invite_join`. The optional body field `keep_ports` (JSON bool): `true` — the `ports.published` publications stay and work in the new network; no field, not a bool and `false` — the publications are turned off, `ports.opened` stays. |
| `POST /api/network/create` | `{"label":"our gaming room"}` | 200 `{"key":"our-gaming-room--aliens-congested-4dd5mwxvn77e6pptnnw01-glbdthzco34lodbfmhh6knwjvto4c7fqz6irhnyn367juokngaya","label":"our-gaming-room","restart":true}` | Creating your own network. The network is always a Managed one: its key carries the creator's node id (`label--word-word-group-<52 characters>`), the creator is its first administrator, and the page advises making a second member an administrator. The full key comes in this response; after that it is given out only by `GET /api/network/key` and the invite `GET /api/network/invite`, and it is not shown on screen a second time. The config changes (the new key goes into the file), `rev` is raised. The `keep_ports` body field is as for `join`: by default the port publications are turned off. A sent `kind` field is ignored. |
| `POST /api/network/inspect` | `{"key":"<key or mesh-invite2-… string>"}` | 200 `{"label":"our-gaming-room","id_hex":"9c1e…"}` | Tells what a pasted key or invite is before joining: the label (`null` if none) and the network identifier. Nothing is joined or written, and an invite is only parsed, never redeemed (it is not spent). An unreadable key or invite is 422 `invalid_key`, as for `join`; a key of the older format (`mesh1-…` or three words with a group) and an invite of the older format are refused the same way with the text "This network key is from an older Mesh Town and no longer works. Ask a network admin for a new invite." or "This invite is from an older Mesh Town. Ask a network admin for a new one.". |
| `POST /api/network/delete` | `{"confirm":true}` | 200, the member list as for `members/leave` | Deleting a network for everyone, by an administrator. Without `confirm: true` — 422 `invalid`. 409 `not_admin`, `network_deleted`. See "Managed networks: members and deletion" below. |
| `GET /api/network/invite` | — | 200 `{"invite":"mesh-invite2-…","qr":"<svg …>…</svg>"}` | For the "Invite a member" button: each call issues a **new single-use** invite, valid for a day (`INVITE_TTL_SECS = 86_400`, `core/invite/head.rs`). The string has no network key: it holds a 32-byte one-time secret, 4 bytes of the network identifier (the newcomer uses them to check that the key came from that network), the issue time, the label and the addresses of this node and up to 4 other members (`MAX_INVITE_PEERS = 5`). The node writes only the secret's hash into the shared network document; the issue is recorded first, then the string is returned. The `qr` field is the same string drawn as a QR code, an SVG document (error correction level L, QR version 25 or smaller); it is `null` when the string is too long for that. Still hand the string to the member in person: whoever presents it first is the one who gets in. 409 `busy` while the node is not in the network (first run, a restart after a network change). |
| `GET /api/network/key` | — | 200 `{"key":"our-gaming-room--aliens-congested-4dd5mwxvn77e6pptnnw01-glbdthzco34lodbfmhh6knwjvto4c7fqz6irhnyn367juokngaya"}` | The page calls it only on the "Show" button of the network key (the key lives in page memory until the network changes); to share the network use an invite (`GET /api/network/invite`). Returns the whole key — for embedding and debugging. |
| `POST /api/token/rotate` | empty | 200 `{"token":"…","url":"http://127.0.0.1:7777/?token=…"}` | A new persistent token (the tray and scripts read it from the `web-token` file). The old one stops working immediately; the page rewrites its own `sessionStorage` itself. It also invalidates all cookie sessions, **except the request's own session**: a session opened by exchanging a leaked token would otherwise outlive the revocation. |
| `POST /api/actions/stop` | empty | 202 `{"stopping":true}` | Stopping the node. After the response the node shuts down; the page goes to the "Mesh Town is stopped" state and stops polling. |
| `POST /api/actions/restart` | empty | 202 `{"restart":true}` | Restarting the node in the same process: the settings file is reread, CLI flags stay. 409 `busy` when the node is not running (offline, starting, stopping). For the primary network without a key (no network yet) the restart applies the pending `socks.*` settings to the process-wide proxy and answers 202 `{"restart":true}`; it is refused with 409 `busy` when no `socks.*` field is pending, when a launch flag sets one, or when the proxy cannot be rebound (the text says why). |

### Managed networks: members and deletion

In a network (`{base} as in section 1.12, `WEB-API-members.md`):

- `POST {base}/members/admin` with `{"id":"<64 hex digits>","admin":true|false}` makes the node an administrator or a regular
  member (only an administrator may; the creator cannot lose the role). A missing id is 400 `bad_request`, a missing or non-boolean
  `admin` is 400 `bad_request`, a malformed id is 422 `invalid`. The reply is the member list with `changed`.
- `exclude` is one administrator's decision.
- `POST {base}/network/delete` with `{"confirm":true}` (without it 422 `invalid`) closes the network for everybody; the members leave
  it, this node too. Reply as for `members/leave`.
- Further 409 refusals: `not_admin` (the action needs an administrator, this includes `network/invite`: 409 `{"error":"not_admin","message":…}`),
  `last_admin` (the only administrator cannot give up the role), `network_deleted` (the network was closed).

### The `node.invite_join` field and single-use invites

In `GET /api/state`, in the `node` section, the `invite_join` field is `null` (no join by invite) or an object
`{"state": "waiting"|"refused", "message": "…", "label": "…"|null, "expires_unix": 1234567890}`:

- `waiting` — the node is waiting for network members (the text "Waiting for network members: as soon as someone from the invite
  is online, the node will join the network."); `expires_unix` — when the invite expires (issue + a day);
- `refused` — a final refusal, `message` is the text for a person: another node has already joined (the invite is
  single-use), the members do not know the invite (revoked, expired, string corrupted), the time ran out,
  a member answered with a key of another network. The page shows `message` as is.

The field disappears (`null`) when the node got the key and joined the network, and also on a new `join`, network creation
or node stop. A refusal stays visible until a new join replaces it.

**A known limit.** An invite is spent by a mark in the shared network document, and the document is replicated between
members in tens of milliseconds. If two members get a presentation of the same string within this window,
both may hand out the key: two nodes will join with one string. The harm is limited — the network key is the same, and
two people joining with one string equals one extra member; rereading the marks does not close the window. Two
simultaneous presentations to the same member do not cause a race: it answers with the key to only one.
If the spent mark is written but the key did not arrive (response timeout 5 s, connection break), the invite is spent
without a join: the newcomer will see `Internal`, then `Used` (the text "another node has already joined").

What is left open in the contract: what to do with fields that require a restart on `import`.

---

## 2. Config schema

The whole `config` document. Units are seconds and bytes, with no human strings.

```json
{
  "version": 1,
  "node":      { "name": "finch-unwary" },
  "network":   { "label": "our-gaming-room",
                 "masked": "our-gaming-room--••••••••••••••••••••••••••••••••••" },
  "exit":      { "enabled": true, "max_streams": 0 },
  "socks":     { "enabled": true, "port": 1080, "strict": false },
  "checks":    { "enabled": true, "interval_sec": 900,
                 "doh_server": "1.1.1.1:443", "provider_dns": null },
  "probe":     { "interval_sec": 300, "full_cycle": false },
  "route":     { "exits": [] },
  "ports":     { "published": [], "opened": [] },
  "web":       { "enabled": true, "port": 7777, "access": "localnet" },
  "log":       { "ring_lines": 2000 },
  "autostart": { "enabled": false, "available": false,
                 "reason": "will appear together with the service" },
  "full_tunnel": { "enabled": false, "available": false,
                 "reason": "will appear together with TUN" }
}
```

| Path | Type | Default | Constraints | Restart | Source | Where on the page |
|---|---|---|---|---|---|---|
| `version` | integer | 1 | exactly 1; anything else — 422 `invalid` | — | — | nowhere |
| `node.name` | string | generated by the node on first start: two words from the `core/members/words.rs` pool (example: `finch-unwary`) | non-empty after trimming whitespace, at most 64 characters; letters of any script are explicitly allowed, there are no other constraints; an empty string from the page — the node substitutes a random name itself (not 422); a violation — 422 `invalid` with `fields`, like the other fields of section 1.7 | no | — | first screen → "This computer's name" (`#home-name`) |
| `network.label` | string | — | read-only | — | — | "Diagnostics" → "Mesh Town network key"; "Home" → the "Network" card |
| `network.masked` | string | — | read-only; the label, `--` and one `•` per character of the tail (34 for the `finch-unwary` example) | — | — (the key is not shown) | "Diagnostics" → "Mesh Town network key" |
| `exit.enabled` | boolean | `true` | — | no | `core/node/config.rs`, `core/cli/commands.rs` | "Home" → "This computer" → "Allow other members to open sites through this computer"; "Members" → a switch on the own row; "Home" → the "Exit via…" card |
| `exit.max_streams` | integer | 0 | 0…100000; `0` = by machine memory (20 on ≤128 MiB, 50 on ≤512 MiB, 200 otherwise); process-wide, shared by all networks | no (on the fly; open streams are not cut) | `core/exit/limits/`, `core/node/node_ctl/live_paths.rs` | no input field |
| `socks.enabled` | boolean | `true` | — | **yes** (the port is taken at startup, `ARCHITECTURE.md`, "How the node works", item 1) | `core/node/config.rs`, `core/cli/commands.rs` (the `--no-socks` flag) | "Settings", the "Browser proxy" row — next to the port; "Home" → the "Browser proxy" card |
| `socks.strict` | boolean | `false` | — | no (applies at once) | `core/proxy/pac.rs` | "Settings" → "Strict mode" |
| `socks.port` | integer | 1080 | 1024…65535; a busy port — 422 | **yes** | `core/node/config.rs`, `core/cli/commands.rs` | "Settings" → "Proxy port (SOCKS5 and HTTP)"; the port belongs to the process (one proxy listener for all networks), not to a slot |
| `proxy.net` | string: `main` or a network id | `"main"` | 6 characters a-z, 2-7 or `main`, otherwise 422; in the primary config an id of a network that is not on the node — 404 `unknown_net`, the file is untouched | no (the proxy switches to the named network's exit at once) | `core/proxy/front/`, `core/web_gui/routes/config.rs` | not shown |
| `checks.enabled` | boolean | `true` | — | no | `core/node/config.rs`, `core/cli/commands.rs` (the `--no-checks` flag) | "Settings", the "Check sites from the list" row — next to the interval |
| `checks.interval_sec` | integer | 900 | 30…86400; list on the page: 300, 900, 1800, 3600, 21600 | no | `core/cli/commands.rs` | "Settings" → "How often to check sites" |
| `checks.doh_server` | string `address:port` | `"1.1.1.1:443"` | parsed as `SocketAddr` | **yes** | `core/cli/commands.rs`, `core/node/config.rs` | not shown |
| `checks.provider_dns` | string `address:port` or `null` | `null` | the same or `null` | **yes** | `core/cli/commands.rs`, `core/node/config.rs` | not shown |
| `probe.interval_sec` | integer | 300 | 5…86400; list on the page: 60, 300, 600, 1800, 3600 | no | `core/cli/commands.rs`, `core/node/config.rs` | "Settings" → "How often to check connection speed" |
| `route.exits` | list of strings (full node ids) | `[]` | no repeats, up to 64 entries; order = priority; an unparsed id/repeat/overflow — 422 `invalid` | no (on the fly — `RouteCtx::set_chosen_exits`, without reconnects) | `core/proxy/route.rs`, `core/node/config.rs` | "Nodes" — a checkbox/order of the selected exits in the member list |
| `ports.published` | list of objects `{port, proto, name, for}`: `port` 1…65535, `proto` `"tcp"` (default; written to the file only for UDP) or `"udp"`, `name` a string up to 64 characters (none — empty), `for` — `"all"` or a list of full node ids (up to 64) | `[]` | `for` is required (missing or another value is not "all" but a refusal); no repeated `(proto, port)`, up to 64 entries; any corruption makes the whole list invalid: 422 `invalid` on PUT (text: `a list of publications {port, proto, name, for} without repeated (proto, port), up to 64, proto is "tcp" (default) or "udp", for is "all" or a list of ids`), `[]` and a line in `field_errors` when reading the file | no (on the fly — the node's publication table, without reconnects) | `core/ports_sharing/`, `core/node/config.rs` | "Ports" — own publications |
| `ports.opened` | list of objects `{node, port, proto, local}`: `proto` as above; the full node id, its port, the listener's local port (1…65535) | `[]` | no repeats of `(node, proto, port)` and `(proto, local)`, up to 64 entries; a new `local` already opened in another network of this node — 422 `local port L is already opened in network "<name>"` (`local UDP port L …` for UDP) (a conflict already in the file does not block other fields); any corruption makes the whole list invalid, as above (text: `a list of {node, port, proto, local} without repeats of (node, proto, port) and (proto, local), up to 64, proto is "tcp" (default) or "udp"`) | no (on the fly — the listeners of opened ports) | `core/ports_sharing/`, `core/node/config.rs` | "Ports" — other members' ports opened here |
| `web.enabled` | boolean | `true` | read-only (in `read_only`): the page does not turn itself off, otherwise there would be nothing to turn it back on with | — | web listener | not shown |
| `web.access` | string `"localhost"` or `"localnet"` | `"localnet"` | an unknown value — 422 `invalid`; **not** in `restart_fields`: narrowing (`localnet` → `localhost`) takes effect at once (the node stops admitting peers from the network), widening with a socket on `127.0.0.1` takes effect after the process restarts, and the divergence is visible in `GET /api/auth/status` (`access` ≠ `listening`, `WEB-API.md` 1.10). The "Restart" button (a network restart) does not reopen the page listener | — | `core/web_gui/auth/access.rs` | "Settings" → "Who can access the settings page" (explanation: this is only about access to the node settings, not to the network or traffic) |
| `web.port` | integer | 7777 | 1024…65535; read-only (in `read_only`, which is why it is not in `restart_fields`) | — | — | page header, no input field |
| `log.ring_lines` | integer | 2000 | 200…20000 | no | — (2,000 lines) | caption under the log window, no input field |
| `node.memory_warn_mib` | integer | 400 | 0…1048576, MiB; `0` = off; `400` means "by machine memory" (24 on ≤128 MiB, 64 on ≤512 MiB, 400 otherwise), so an explicit 400 on a machine with ≤512 MiB also counts as automatic; on such machines the exit also refuses new streams near this value, above 512 MiB with the default 400 it refuses only by the stream limit | file only | `core/node/host/process.rs` | not shown |
| `autostart.enabled` | boolean | `false` | `true` with `available: false` → 422 `not_available` | — (cannot be enabled) | — | "Settings", the switch is gray |
| `autostart.available` | boolean | `false` | read-only | — | — | controls the gray look of the switch |
| `autostart.reason` | string | "will appear together with the service" | read-only | — | — | caption under the switch |
| `full_tunnel.enabled` | boolean | `false` | as for `autostart.enabled` | — (cannot be enabled) | — | "Settings", the switch is gray |
| `full_tunnel.available` | boolean | `false` | read-only | — | — | gray look |
| `full_tunnel.reason` | string | "will appear together with TUN" | read-only | — | the same | caption |

`autostart.enabled` and `full_tunnel.enabled` are stored in the file (these are settings that a person
may set in advance), while `autostart.available`, `autostart.reason`, `full_tunnel.available`,
`full_tunnel.reason` are not: these are build flags, and the node fills them itself on every start.

What is **not** in the config and why: the page token, the network key, the node's
domain list, `blocked_domains` (`core/node/config.rs` — a debug flag, there is no place for it on the page).

---

