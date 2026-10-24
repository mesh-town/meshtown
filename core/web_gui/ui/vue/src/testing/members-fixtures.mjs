/* Builders of `GET /api/members` replies for page tests. The shape follows the member-list API contract
 * (rows with `status`, `vote`, `excluded`, `blocked_by_me`, `denied_by_me`, `last_refusal`; top level `me`, `me_excluded`,
 * `need`, `counts`). The ids are the ones of `baseState` / `demoState`, so a members reply lines up with `state.peers`:
 * `SELF_FULL` ("You"), `PEER_FULL` (peer-one), `PEER2_FULL` (a third node); `EXTRA_FULL` is a fourth node that is not in `state.peers`. */

import { PEER2_FULL, PEER_FULL, SELF_FULL } from "./section.mjs";

export const EXTRA_FULL = "D".repeat(64);
export const AT_UNIX = 1792400000; // 2026-10-19, UTC

export const memberRow = (over = {}) => ({
  id: "xxxx0000", id_full: EXTRA_FULL, name: null, self: false, online: true, status: "member",
  vote: null, excluded: null, blocked_by_me: false, denied_by_me: null, last_refusal: null, admission: null, ...over,
});

export const selfRow = (over = {}) => memberRow({ id: "aaaa1111", id_full: SELF_FULL, name: "finch-unwary", self: true, ...over });
export const peerOneRow = (over = {}) => memberRow({ id: "bbbb2222", id_full: PEER_FULL, name: "peer-one", ...over });
export const peerTwoRow = (over = {}) => memberRow({ id: "cccc3333", id_full: PEER2_FULL, name: null, online: false, ...over });
export const extraRow = (over = {}) => memberRow({ id: "dddd4444", id_full: EXTRA_FULL, name: "old-laptop", ...over });

// A pending ban vote on a row; by default proposed by "You" and counted as mine.
export const voteInfo = (over = {}) => ({ votes: 1, need: 2, proposed_by: SELF_FULL, voters: [SELF_FULL], mine: true, reason: null, ...over });

// A vote proposed by someone else (peer-one) that "You" has not joined yet: "You" can confirm it.
export const foreignVote = (over = {}) => voteInfo({ proposed_by: PEER_FULL, voters: [PEER_FULL], mine: false, ...over });

// A join request: one confirmation of two, not mine by default; the id is the 4th node (not in `state.peers`).
export const admissionInfo = (over = {}) => ({ votes: 1, need: 2, voters: [PEER_FULL], mine: false, label: null, ...over });

export const pendingRow = (over = {}) =>
  memberRow({ id: "eeee5555", id_full: "E".repeat(64), name: null, online: false, status: "pending", admission: admissionInfo(), ...over });

export const excludedInfo = (over = {}) => ({ how: "votes", at_unix: AT_UNIX, voters: [SELF_FULL, PEER_FULL], ...over });

// The whole reply; `counts` are computed from `rows` unless overridden.
export function membersReply({ rows = [selfRow(), peerOneRow(), peerTwoRow()], ...over } = {}) {
  const count = (pred) => rows.filter(pred).length;
  return {
    me: SELF_FULL,
    me_excluded: false,
    me_pending: false,
    me_admission: null,
    need: 2,
    counts: {
      members: count((r) => r.status === "member"),
      voting: count((r) => r.vote !== null),
      excluded: count((r) => r.status === "excluded"),
      unlisted: count((r) => r.status === "unlisted"),
      pending: count((r) => r.status === "pending"),
    },
    members: rows,
    ...over,
  };
}

// An action reply: the new reply plus `changed`.
export const actionReply = (reply, changed = true) => ({ ...reply, changed });

// `/api/state` with the members summary of the contract (`voting`, `me_excluded`) merged into `state.members`.
export const withMembersSummary = (state, summary = {}) => ({
  ...state,
  members: { count: state.peers.length, gossip_neighbors: 0, voting: 0, me_excluded: false, ...(state.members || {}), ...summary },
});
