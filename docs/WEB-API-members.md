# Node HTTP API contract: members, continued (4)

Part 4 of 4. A continuation of section "1. HTTP API contract" (subsection 1.12). Part 1 (1.1-1.5, 1.10, 1.11) is
`WEB-API.md`. Part 2 (1.6-1.8, section 2) is `WEB-API-config.md`. Part 3 (section 3) is `WEB-API-embedding.md`.

---

### 1.12 Members: `GET {base}/members` and the member actions

`{base}` is `/api` for the primary network or `/api/net/{slot}` for any other (section 1.11); the network is chosen
only by the prefix, no body carries a network. An unknown slot is 404 `unknown_net`.

**`GET {base}/members`** — 200, the member list as this node sees it:

| Field | Meaning |
|---|---|
| `me` | the full id of this node |
| `me_excluded` | the members banned this node |
| `me_pending`, `me_admission` | this node waits for an administrator to admit it; its request while it waits, else `null` |
| `counts` | `{"members","excluded","unlisted","pending"}` |
| `members` | the rows, see below |
| `changed` | only in replies to actions: whether the action changed anything |
| `admins`, `me_admin`, `root` | the full ids of the administrators in ascending order; whether this node is one; the creator of the network |

A row of `members`: `id` (short), `id_full` (64 hex digits), `name`, `self`, `online`, `status`, `excluded`,
`blocked_by_me`, `denied_by_me`, `last_seen_sec`, `last_refusal`, `admission`, `admin`.

`admin` is `true` for an administrator of the network.

- `status` is one of `member`, `pending`, `unlisted`, `excluded`; rows are ordered in that way after the
  own row.
- `denied_by_me` is `null` or why this node refuses the row: `left`, `excluded`, `blocked`, `removed`, `unlisted`.
- `last_seen_sec` is the number of seconds since the last sign of life of the node (a connection, telemetry, the
  address cache or a write in the shared document); 0 for a node connected now, `null` when there is no sign at all.

**Actions.** All are `POST {base}/members/<name>` with `<name>` one of `exclude`, `unexclude`, `admit`,
`block`, `unblock`, `leave`, `admin`. `exclude`, `unexclude`, `admit`, `block` and `unblock` take the body `{"id":"<64 hex digits>"}` (`exclude` also an optional `reason`); a missing `id` is 400 `bad_request`, a
malformed one 422 `invalid`. `leave` takes the body `{"confirm":true}` (without it 400 `bad_request`). The reply is 200 with the member list after the action, with
`changed`.

Refusals are 409 with `error` set to the code: `self_target` (own id), `not_member` (the id is not in the list),
`left` (this node left the network; a node the others removed from the list gets the same refusal for every action except `leave`, `unexclude` and `unblock`), `not_pending`. A write failure is 500 `internal`.

`members/admin`, `network/delete` and the administrator refusals are described in "Managed networks: members and deletion" in `WEB-API-config.md`.

**What each action means.** The reply and the body are the ones described above; only the effect differs.

- `exclude` — the decision of an administrator to remove the node (details in
  "Managed networks: members and deletion" in `WEB-API-config.md`). The page calls it "Remove from network…".
- `unexclude` — withdraws this node's own `exclude` entry in the shared document; other members' entries stay. It is
  `changed:false` when there is nothing to withdraw; there is no "nothing to withdraw" refusal. The page calls it
  "Withdraw proposal".
- `admit` — an administrator lets in a node that waits to join; it is also how an administrator returns an excluded node. Refused with `not_pending` if the
  node does not wait for admission. The page calls it "Confirm…".
- `block` and `unblock` — a **local** block: the entry is written only to this node's own replica, never to the shared
  document, so the node stays in the network and keeps working with the other members. No administrator role is
  needed. `blocked_by_me` in the row shows it. The page calls them "Block on this computer…" and "Unblock".
  `unblock` of a node that is not blocked is `changed:false`.

`exclude`, `unexclude` and `admit` need an administrator (409 `not_admin`);
`block` and `unblock` do not. Excluding the creator of the network or taking the role from it is refused
with 409 `creator`: the creator always stays an administrator. A node id in a request is 64 hex digits (a placeholder: 64 times `a`).
