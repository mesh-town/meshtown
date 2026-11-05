# State snapshot field notes

Part of the HTTP API contract: a continuation of section 1.3 `GET /api/state` in `WEB-API.md`, which holds the
example snapshot. The same rules apply: the node, the demo node and the frontend must match field names, error
codes and enum values character for character.

Exact enum values and field notes, by group.

**Node (`node.*`)**

- `node.status`: `"starting"`, `"online"`, `"offline"`, `"stopping"`. `"offline"` also covers a node
  started without a network key and waiting for a network (`network: null`, below).
- `node.start_error`: a string with the reason when the last start (a join or a restart) failed and the process
  went back to waiting for a network (`status` is `"offline"`); `null` otherwise. A new start clears it.
- `node.version` — the binary version string, shown in the header caption next to the address.
- `node.id_full` — the full node identifier, shown only as a tooltip (`title`) on the own row in "Networks"; it is
  not in the page text.
- `node.identity`: `"persistent"` — the node saved its identity and stays the same network member after a restart;
  `"temporary"` — it did not save it, and after a restart other members see this computer as a new member.
  With `"temporary"`, `node.identity_reason` is filled in; otherwise it is `null`. Its values:
  - `"declined"` — the identity is not saved by explicit instruction (the node was started with `--no-config`).
  - `"file_preserved"` — the file on disk is **intact and deliberately left untouched**. Overwriting would have lost
    the user's data, and making a `config.json.bad-<label>` copy failed, so the node left the original as is. The
    file may contain the network key.
  - `"file_problem"` — any other file trouble: nowhere to write, the file is unreadable or corrupt, no place for the
    file at all. For the user this is all the same ("identity not saved because of the file").

  `file_preserved` is separate from `file_problem` because of what the user should do. For `file_problem` the usual
  advice is "delete or fix the file". Here that advice is destructive: the file is intact and may hold the only copy
  of the network key. So the page shows a separate text — the file is not broken, do not delete it.

  The page takes the wording of the reason not from this value but from the `file` block (`message` and
  `field_errors`, section 1.6, `WEB-API-config.md`), where the reason is written in plain language.
  `identity_reason` answers one question: did the user decide this, or is it a failure they do not know about? The
  first is reported calmly, the second needs attention.

  The flag comes from the config layer's `FileReport` (`identity_declined` and `overwrite_refused` next
  to `persistent_identity`, `CONFIG-FILE.md`). The page gets all three values as **structured fields, not by
  parsing the text** of `message`: the text is written for a person and changes freely, and behavior must not
  depend on it.
- `node.invite_join` — `null` or `{"state": "waiting"|"refused", "message", "label", "expires_unix"}`:
  progress of joining by a single-use invite (`POST /api/network/join` with a `mesh-invite2-…` string).
  `waiting` — the node is looking for members named in the string; `refused` — a final refusal with the text
  `message`. The field disappears when the node has joined the network, and on a new join, network creation or
  stop. More in `WEB-API-config.md`, section "The `node.invite_join` field and single-use invites".
- `boot_id` — random 8 hexadecimal characters, new on every process start. A changed `boot_id` means the node
  restarted: the page clears the log and rereads the config.
- `now` — seconds since node start with one decimal place, the same scale as the log lines
  (`core/node/output.rs`).

**Network (`network.*`, `members.*`)**

- `members.closed`: `true` when an admin has closed the network to new members (read from the access ledger);
  `false` while the node is not attached to a network.
- `network` — `null` when the node has no readable key. On the very first run there is no network yet: the user has
  not created or joined one. Then the "Home" page shows the panel "You haven't joined a Mesh Town network yet" with
  the buttons "Join with an invite" and "Create a new network"; the node does not create a network itself.
  Otherwise, besides `label`, `masked` and `id_hex`, it says who runs the network and what this node may do in it:
  - `root`: the full id of the network creator (an admin who cannot lose the role).
  - `me_role`: `"admin"` or `"member"`.
  - `admins`: full ids of the admins in ascending order.
  - `can_invite`, `can_remove`, `can_set_admins`, `can_delete_network`, `can_leave`: the actions open to this node.
    The first four are `true` for an admin only. A node with no member list yet gets the flags from the key alone:
    all `false` except `can_leave`. The page shows no control whose flag is off.
  - `excluded_by`: `{"id": "<id_full>", "name": "…"}` when an admin excluded this node (`name` is the known
    member name, else the short id); `null` otherwise, and after the node left on its own.
  - `excluded_reason`: the note given with that removal (a string), or `null` when none was given, when the node is
    not excluded by an admin, or after it left on its own.

**Peers (`peers[]`)**

- `peers[].name` — the node name (`config.node.name`, section 2, `WEB-API-config.md`), its own or one changed by the
  user, including non-Latin scripts. The field is **optional**: the name may be absent or empty, and then the page
  shows `id`. The own name is always there. Others' names arrive through the shared network document and are
  matched with peers with a delay, so for the first minutes after joining some members have no names; the page
  must cope with that. The page shows the name in "Networks" instead of the bare `id`. The tooltip (`title`) with
  the full `id_full` stays only on the own row.
- `peers[].id_full` — the full identifier of **each** node, not only the own one (unlike `node.id_full`). It is
  needed as a `route.exits` value when choosing exits on the page and in the tray (the checkbox puts `p.id_full`
  into the list), not only for the tooltip. Other rows do not show it in the page text.
- `peers[].role` — `"admin"` or `"member"`; present only in a Managed network.
- `peers[].path`: `"direct"`, `"relay"`, `null` (no path, or it is this node).
- `peers[].exit`: on the own row, `true`/`false` — whether exit is enabled on this node. On other rows,
  `true`/`false` — as announced by the node itself (the `node/exit/` key in the shared network document); `null` —
  the node is not in the network or there is no flag (a build without it, or the announcement has not arrived).
  The page shows "—" for `null`.
- `peers[].through_peer_bps` — integer, bytes/s: our traffic through this peer as an exit (outgoing
  EXIT connections to it; sent + received over a sliding 10 s, divided by 10). Not counted: direct exit, port
  forwarding (`PORT`), volume probes and sync. Always present; `0` on the own row and when there is no data. An old
  node does not send the field; the reader treats its absence as `0`.
- `peers[].through_us_bps` — the same for this peer's traffic through our exit (incoming EXIT connections from it).
  The window, composition and defaults are the same as for `through_peer_bps`.
- `peers[].version` — the member's Mesh Town program version, a string like `0.1.0` (digits, letters,
  `.`, `-`, `+`, up to 32 characters). The own row has the version of this build, like `node.version`. Other nodes
  announce it with the `node/version/` key in the shared network document at startup, if the recorded one differs.
  `null` — the member has not announced it (a build before this field, or the record has not arrived) or announced
  an invalid one. An offline member shows the last announced version. Comparing versions is the page's concern.

**Exit and route**

- `exit.interface` — `null` when exit traffic uses the regular routing table, otherwise `{"name": "wg0", "state": …}`,
  where `state` is `up`, `down`, `missing` (no such interface on this machine) or `unsupported` (this system cannot
  bind to an interface). Anything but `up` means exit connections are refused (reply code 8).
- `exit.clients` — full ids (`id_full`) of members who currently reach the internet through this node: there is an
  open exit stream, or it closed less than 30 s ago. Network members only, in ascending order; `[]` — no clients or
  exit is off.
  - A client that dropped without closing the connection (a powered-off node, a network break) disappears after
    32–35 s. The node removes it from network members on connection loss, and the list shows only members, so the
    30-second QUIC wait is not added on top of the window.
  - A client stopped by a signal disappears within a second.
  - A client that left and came back within 30 s is visible in the list again until the end of the window, even
    with no more traffic.
- `route.state`: `"direct"` (`route.exits` is empty — the node goes out itself); `"exit"` (the list is not empty and
  has at least one suitable candidate — connections go through the first one); `"unavailable"` (the list is not
  empty and has no suitable candidates — the node refuses all new connections). The selection rule is the same
  `route::choose_route` function as in the proxy.
- `route.exits[]` — the whole `route.exits` list in config order (not only the selected candidate), with the current
  status of each node:
  - `id`/`id_full` — identifier;
  - `name` — the same value as `peers[].name` for this id (`null` if it has not arrived yet);
  - `online` — whether it is online;
  - `exit` — whether its "works as an exit" flag is on (`null` — unknown);
  - `active` — whether this node is currently the first suitable candidate (`true` for at most one element);
  - `removed` — present and `true` when the network's member list holds an exclusion of this node (it is no longer
    in the network; such an exit is also `online: false`).
- `domains[].checks[<id>].verdict`: `"ok"`, `"dns_mismatch"`, `"network_blocked"`, `"site_geoblock"`, `"antibot"`,
  `"down"` — exactly six values from `core/site_checks/reachability/verdict.rs`. `null` instead of an object — there
  is no record from this member. The connection path does not depend on these checks; the snapshot has no
  `domains[].route`.

**Transport**

- `transport.self_path`: `"direct"`, `"relay"`, `null` — how this machine currently reaches the network. The values
  are the same three as for `peers[].path`, so the page parses them with one function.
- `transport.relay.serving`: `true` while this node's own relay carries a pair of other members (a session is open,
  or a pairing was completed since the previous check, about every 30 seconds); `false` otherwise and when there is
  no relay. It is a flag, not a count. An older node omits the key: read it as `false`.
- `transport.dht.nodes`: the number of nodes in the DHT routing table, read once per directory round; `0` means the
  table is empty. If a round cannot read it, the previous value stays.
- `transport.record.members`: the number of keys in the last roster read from the DHT, this node included. The key
  is absent until the first read.

**Ports (`ports.*`)**

- `ports.published[]` — own publications. `proto`: `"tcp"` or `"udp"`. `local`: `"unknown"` (not checked), `"ok"`
  (the port responds), `"refused"` (does not respond); for UDP it is always `"unknown"`.
- `ports.network[]` — ports of other nodes: entries with the scope "all" and a readable value while the author is in
  the network, plus everything opened here. Own publications are not in `network`. A row is identified by
  `(node, proto, port)`: TCP 53 and UDP 53 of one node are two rows. Fields:
  - `proto` — `"tcp"` or `"udp"` in every row.
  - `node` — the full identifier.
  - `online` — the same flag as in `peers[]`.
  - `published: false` — the port is open here, but there is no valid record for everyone.
  - `opened` — `state` is `"binding"` (a UDP socket is still being taken), `"listening"` or `"bind_failed"`;
    `error` is the bind error text, only with `bind_failed`.
  - `last_error` — the outcome of the last failed attempt to reach the publishing node: `null` (none yet, or the
    last attempt succeeded), `"offline"`, `"not_opened"`, `"no_service"`, `"timed_out"`, `"failed"` or
    `"unsupported"` (the node does not support UDP ports). For UDP the attempt is made when a new sender's first
    datagram arrives.
  - `web_url` — always `null` for UDP. Otherwise the link to the port at the member's mesh address
    (`http://<name>.meshtown`, with `:port` unless the port is 80, `https://` for 443). `null` when the port is
    not shared with this node (`published: false`) or no proxy answers mesh addresses.

**Log**

- `log.tail` — exactly the last 6 lines, for the "Recent events" card on "Home".
