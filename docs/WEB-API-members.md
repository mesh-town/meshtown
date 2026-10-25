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
| `me_removed` | the members removed this node from the member list (a removed node is told so when it dials a member) |
| `me_pending`, `me_admission` | this node waits for confirmations; its request while it waits, else `null` |
| `need` | votes the network needs for a decision: 1 while at most two nodes are active, else 2 |
| `counts` | `{"members","voting","excluded","unlisted","pending","forgotten"}`; removed nodes are only in `forgotten` |
| `members` | the rows, see below |
| `changed` | only in replies to actions: whether the action changed anything |
| `kind` | `"Peer"` or `"Managed"`, from the rules of the network (a Managed network without an administrator still says `"Managed"`) |
| `admins`, `me_admin`, `root` | the full ids of the administrators in ascending order (`[]` for Peer); whether this node is one; the creator of a Managed network, else `null` |

A row of `members`: `id` (short), `id_full` (64 hex digits), `name`, `self`, `online`, `status`, `vote`, `excluded`,
`blocked_by_me`, `denied_by_me`, `forget`, `last_seen_sec`, `last_refusal`, `admission`, `admin`.

`admin` is `true` for an administrator of a Managed network (always `false` in Peer).

- `status` is one of `member`, `pending`, `unlisted`, `forgotten`, `excluded`; rows are ordered in that way after the
  own row. `forgotten` is a node removed from the member list by enough votes.
- `forget` is `null` or the circle of votes to remove the node: `{"votes","need","proposed_by","voters":[…],"mine",
  "at_unix"}`. `votes` counts the members who voted, `voters` and `proposed_by` are short ids, `mine` — this node has
  voted, `at_unix` — when the node was removed (`null` while the circle waits for more votes).
- `denied_by_me` is `null` or why this node refuses the row: `left`, `excluded`, `voted_by_me`, `removed_by_me` (this
  node voted to remove it, the circle is still open), `blocked`, `removed`, `unlisted`.
- `last_seen_sec` is the number of seconds since the last sign of life of the node (a connection, telemetry, the
  address cache or a write in the shared document); 0 for a node connected now, `null` when there is no sign at all.

**Actions.** All are `POST {base}/members/<name>` with `<name>` one of `exclude`, `unexclude`, `admit`, `unadmit`,
`block`, `unblock`, `forget`, `unforget`, `leave`, `admin`. `exclude`, `unexclude`, `admit`, `unadmit`, `block`, `unblock`,
`forget` and `unforget` take the body `{"id":"<64 hex digits>"}` (`exclude` also an optional `reason`); a missing `id` is 400 `bad_request`, a
malformed one 422 `invalid`. `leave` takes the body `{"confirm":true}` (without it 400 `bad_request`). The reply is 200 with the member list after the action, with
`changed`.

- `forget` — this node votes to remove a node that is not connected from the member list. The vote is recorded as a
  signed entry of this node; the node is removed when the votes reach `need` (a second member repeats `forget` for
  the same id). The proposer hides the node at once; it is hidden for the others when the circle completes.
  Repeating the same vote is `changed:false`.
- `unforget` — withdraws this node's vote; when the votes fall below `need` the node is a member again for everyone.
- A new admission of the node (an invitation) made after the votes ends the circle: those votes no longer count.

Refusals are 409 with `error` set to the code: `self_target` (own id), `not_member` (the id is not in the list),
`left` (this node left the network; a node the others removed from the list gets the same refusal for every action except `leave`, `unforget`, `unexclude` and `unblock`), `not_pending`, `not_confirmed` (`admit`, `unadmit`), `online` (`forget` of a node
that is connected now), `not_forgotten` (`unforget` without a vote of this node). A write failure is 500 `internal`.

Managed networks add `members/admin`, `network/delete` and more refusals: see "Managed networks: members and deletion" in `WEB-API-config.md`.

**What each action means.** The reply and the body are the ones described above; only the effect differs.

- `exclude` — this node's vote to ban the node, or in a Managed network the decision of an administrator (details in
  "Managed networks: members and deletion" in `WEB-API-config.md`). The page calls it "Ban from network…" (Peer),
  "Remove from network…" (Managed) and, for the second vote, "Confirm ban…".
- `unexclude` — withdraws this node's own `exclude` entry in the shared document; other members' entries stay. It is
  `changed:false` when there is nothing to withdraw; there is no "nothing to withdraw" refusal. The page calls it
  "Withdraw proposal".
- `admit` — confirms a node that waits to join (a vote for its admission); the node joins when the votes reach `need`.
  In a Managed network it is also how an administrator returns an excluded node. Refused with `not_pending` if the
  node does not wait for this node's confirmation. The page calls it "Confirm…".
- `unadmit` — takes this node's own confirmation back while the node has not joined yet. Refused with
  `not_confirmed` if this node has not confirmed it or the node has already joined. The page calls it "Withdraw
  confirmation" (Peer networks only).
- `block` and `unblock` — a **local** block: the entry is written only to this node's own replica, never to the shared
  document, so the node stays in the network and keeps working with the other members. No administrator role is
  needed. `blocked_by_me` in the row shows it. The page calls them "Block on this computer…" and "Unblock".
  `unblock` of a node that is not blocked is `changed:false`.

In a Managed network `exclude`, `unexclude`, `admit` and `unadmit` need an administrator (409 `not_admin`);
`block` and `unblock` do not. A node id in a request is 64 hex digits (a placeholder: 64 times `a`).
