/* `members-view.ts`: `partitionRows`. The contract is in `members-view.test.mjs`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import {
  canConfirm,
  confirmable,
  displayNames,
  excludedVariant,
  isMeExcluded,
  meId,
  memberById,
  partitionRows,
  excludedRowLine,
  forgottenRowLine,
  inForgottenGroup,
  isStale,
  shortDate,
  STALE_AFTER_SECS,
} from "./members-view.ts";
import { PEER2_FULL, PEER_FULL, SELF_FULL } from "../testing/section.mjs";
import {
  AT_UNIX,
  EXTRA_FULL,
  excludedInfo,
  extraRow,
  foreignVote,
  forgetInfo,
  membersReply,
  peerOneRow,
  peerTwoRow,
  selfRow,
  staleRow,
  voteInfo,
} from "../testing/members-fixtures.mjs";

const ME = SELF_FULL;
const peer = (idFull, over = {}) => ({ id: idFull.slice(0, 8).toLowerCase(), id_full: idFull, name: null, online: true, ...over });
const ids = (rows) => rows.map((r) => r.id_full);

describe("partitionRows", () => {
  describe("members are not known (an old node or a failed request)", () => {
    it("every peer stays in the active list as it is", () => {
      const peers = [peer(PEER_FULL), peer(PEER2_FULL)];
      const parts = partitionRows(peers, null, ME);
      assert.deepEqual(parts.active, peers);
      assert.equal(parts.active[0], peers[0], "the very same peer objects");
      assert.deepEqual(parts.unlisted, []);
      assert.deepEqual(parts.excluded, []);
      assert.deepEqual(parts.forgotten, []);
      assert.deepEqual(parts.stale, []);
    });

    it("no peers either: three empty lists", () => {
      assert.deepEqual(partitionRows(null, null, ME), { active: [], pending: [], unlisted: [], excluded: [], forgotten: [], stale: [] });
    });
  });

  describe("everybody is a member", () => {
    it("the active list is the peers in their order", () => {
      const peers = [peer(PEER2_FULL), peer(SELF_FULL, { self: true }), peer(PEER_FULL)];
      const reply = membersReply({ rows: [selfRow(), peerOneRow(), peerTwoRow()] });
      const parts = partitionRows(peers, reply, ME);
      assert.deepEqual(ids(parts.active), [PEER2_FULL, SELF_FULL, PEER_FULL]);
      assert.equal(parts.unlisted.length + parts.excluded.length, 0);
    });

    it("a pending vote does not move a row anywhere", () => {
      const peers = [peer(SELF_FULL, { self: true }), peer(PEER_FULL)];
      const reply = membersReply({ rows: [selfRow({ vote: foreignVote() }), peerOneRow({ vote: voteInfo() })] });
      assert.deepEqual(ids(partitionRows(peers, reply, ME).active), [SELF_FULL, PEER_FULL]);
    });
  });

  describe("a member was excluded", () => {
    it("its peer row leaves the active list and the row of the reply goes to excluded", () => {
      const peers = [peer(SELF_FULL, { self: true }), peer(PEER_FULL)];
      const gone = peerOneRow({ status: "excluded", excluded: excludedInfo() });
      const parts = partitionRows(peers, membersReply({ rows: [selfRow(), gone] }), ME);
      assert.deepEqual(ids(parts.active), [SELF_FULL]);
      assert.deepEqual(parts.excluded, [gone]);
    });

    it("an excluded member that is not among the peers is still in excluded", () => {
      const gone = extraRow({ status: "excluded", excluded: excludedInfo({ how: "left", voters: [EXTRA_FULL] }) });
      const parts = partitionRows([peer(SELF_FULL, { self: true })], membersReply({ rows: [selfRow(), gone] }), ME);
      assert.deepEqual(ids(parts.active), [SELF_FULL]);
      assert.deepEqual(parts.excluded, [gone]);
    });

    it("excluded keeps the order of the reply", () => {
      const a = extraRow({ status: "excluded", excluded: excludedInfo() });
      const b = peerTwoRow({ status: "excluded", excluded: excludedInfo() });
      const parts = partitionRows([], membersReply({ rows: [selfRow(), b, a] }), ME);
      assert.deepEqual(ids(parts.excluded), [PEER2_FULL, EXTRA_FULL]);
    });
  });

  describe("a node is not in the member list", () => {
    it("its row of the reply goes to unlisted and its peer row leaves the active list", () => {
      const peers = [peer(SELF_FULL, { self: true }), peer(PEER2_FULL)];
      const stray = peerTwoRow({ status: "unlisted" });
      const parts = partitionRows(peers, membersReply({ rows: [selfRow(), stray] }), ME);
      assert.deepEqual(ids(parts.active), [SELF_FULL]);
      assert.deepEqual(parts.unlisted, [stray]);
    });

    it("also when the node is not among the peers; the order of the reply is kept", () => {
      const a = extraRow({ status: "unlisted", online: false });
      const b = peerTwoRow({ status: "unlisted" });
      const parts = partitionRows([], membersReply({ rows: [selfRow(), b, a] }), ME);
      assert.deepEqual(ids(parts.unlisted), [PEER2_FULL, EXTRA_FULL]);
    });
  });

  describe("a member the peer list does not have", () => {
    it("a minimal row after the peers in the active list", () => {
      const peers = [peer(SELF_FULL, { self: true })];
      const parts = partitionRows(peers, membersReply({ rows: [selfRow(), extraRow()] }), ME);
      assert.deepEqual(ids(parts.active), [SELF_FULL, EXTRA_FULL]);
      const minimal = parts.active[1];
      assert.equal(minimal.id, "dddd4444");
      assert.equal(minimal.name, "old-laptop");
      assert.equal(minimal.online, true);
      assert.equal(!!minimal.self, false);
    });
  });


  describe("a member was removed from the list", () => {

    it("the status \"forgotten\" goes to forgotten, not to active", () => {
      const peers = [peer(SELF_FULL, { self: true }), peer(PEER_FULL)];
      const gone = peerOneRow({ status: "forgotten", online: false, forget: forgetInfo({ at_unix: AT_UNIX }) });
      const parts = partitionRows(peers, membersReply({ rows: [selfRow(), gone] }), ME);
      assert.deepEqual(parts.forgotten, [gone]);
      assert.deepEqual(ids(parts.active), [SELF_FULL]);
    });

    it("my own waiting round hides the member from active and shows it in forgotten", () => {
      const peers = [peer(SELF_FULL, { self: true }), peer(PEER_FULL)];
      const waiting = peerOneRow({ online: false, forget: forgetInfo({ mine: true, proposed_by: "aaaa1111", voters: ["aaaa1111"] }) });
      const parts = partitionRows(peers, membersReply({ rows: [selfRow(), waiting] }), ME);
      assert.deepEqual(parts.forgotten, [waiting]);
      assert.deepEqual(ids(parts.active), [SELF_FULL]);
    });

    it("a round of somebody else that I have not joined keeps the member in active", () => {
      const peers = [peer(SELF_FULL, { self: true }), peer(PEER_FULL)];
      const row = peerOneRow({ online: false, last_seen_sec: 60, forget: forgetInfo() });
      const parts = partitionRows(peers, membersReply({ rows: [selfRow(), row] }), ME);
      assert.deepEqual(ids(parts.active), [SELF_FULL, PEER_FULL]);
      assert.deepEqual(parts.forgotten, []);
    });

    it("a forgotten member that is not among the peers is in forgotten and not in active", () => {
      const gone = extraRow({ status: "forgotten", online: false });
      const parts = partitionRows([peer(SELF_FULL, { self: true })], membersReply({ rows: [selfRow(), gone] }), ME);
      assert.deepEqual(parts.forgotten, [gone]);
      assert.deepEqual(ids(parts.active), [SELF_FULL]);
    });

    it("a listed member with a waiting round of mine is not in extra of active either", () => {
      const waiting = extraRow({ online: false, forget: forgetInfo({ mine: true }) });
      const parts = partitionRows([peer(SELF_FULL, { self: true })], membersReply({ rows: [selfRow(), waiting] }), ME);
      assert.deepEqual(ids(parts.active), [SELF_FULL]);
      assert.deepEqual(parts.forgotten, [waiting]);
    });

    it("forgotten keeps the order of the reply", () => {
      const a = extraRow({ status: "forgotten", online: false });
      const b = peerTwoRow({ status: "forgotten" });
      const parts = partitionRows([], membersReply({ rows: [selfRow(), b, a] }), ME);
      assert.deepEqual(ids(parts.forgotten), [PEER2_FULL, EXTRA_FULL]);
    });
  });

  describe("a member has been offline for over a week", () => {
    const WEEK = STALE_AFTER_SECS;
    const peers = [peer(SELF_FULL, { self: true }), peer(PEER_FULL, { online: false })];
    const partsOf = (row) => partitionRows(peers, membersReply({ rows: [selfRow(), row] }), ME);

    it("the week is 604800 seconds", () => {
      assert.equal(WEEK, 604800);
    });

    it("one second over the week: it goes to stale, not to active", () => {
      const row = peerOneRow({ online: false, last_seen_sec: 604801 });
      const parts = partsOf(row);
      assert.deepEqual(parts.stale, [row]);
      assert.deepEqual(ids(parts.active), [SELF_FULL]);
    });

    it("never seen (null) goes to stale too", () => {
      const row = peerOneRow({ online: false, last_seen_sec: null });
      assert.deepEqual(partsOf(row).stale, [row]);
    });

    it("an online member is never stale", () => {
      const row = peerOneRow({ online: true, last_seen_sec: 604801 });
      const parts = partsOf(row);
      assert.deepEqual(parts.stale, []);
      assert.deepEqual(ids(parts.active), [SELF_FULL, PEER_FULL]);
    });

    it("six days stay in active", () => {
      const row = peerOneRow({ online: false, last_seen_sec: 6 * 86400 });
      const parts = partsOf(row);
      assert.deepEqual(parts.stale, []);
      assert.deepEqual(ids(parts.active), [SELF_FULL, PEER_FULL]);
    });

    it("exactly a week stays in active", () => {
      const row = peerOneRow({ online: false, last_seen_sec: WEEK });
      assert.deepEqual(partsOf(row).stale, []);
    });

    it("my own row is never stale", () => {
      const mine = selfRow({ online: false, last_seen_sec: null });
      const parts = partitionRows([peer(SELF_FULL, { self: true })], membersReply({ rows: [mine] }), ME);
      assert.deepEqual(parts.stale, []);
      assert.deepEqual(ids(parts.active), [SELF_FULL]);
    });

    it("a member the peer list does not have is in stale and not in extra of active", () => {
      const row = extraRow({ online: false, last_seen_sec: 900000 });
      const parts = partitionRows([peer(SELF_FULL, { self: true })], membersReply({ rows: [selfRow(), row] }), ME);
      assert.deepEqual(parts.stale, [row]);
      assert.deepEqual(ids(parts.active), [SELF_FULL]);
    });

    it("a member who is not in the list as a member is not stale, whatever its last_seen_sec", () => {
      const row = extraRow({ status: "unlisted", online: false, last_seen_sec: null });
      const parts = partitionRows([], membersReply({ rows: [selfRow(), row] }), ME);
      assert.deepEqual(parts.stale, []);
      assert.deepEqual(parts.unlisted, [row]);
    });

    it("a member with my waiting round is in forgotten only, not in stale", () => {
      const row = peerOneRow({ online: false, last_seen_sec: 900000, forget: forgetInfo({ mine: true }) });
      const parts = partsOf(row);
      assert.deepEqual(parts.forgotten, [row]);
      assert.deepEqual(parts.stale, []);
    });

    it("a round of somebody else on a stale member leaves it in stale", () => {
      const row = peerOneRow({ online: false, last_seen_sec: 900000, forget: forgetInfo() });
      const parts = partsOf(row);
      assert.deepEqual(parts.stale, [row]);
      assert.deepEqual(parts.forgotten, []);
    });

    it("no row is in two groups", () => {
      const rows = [selfRow(), staleRow("old-desktop", 777600), staleRow("ghost-key", null), peerOneRow({ status: "forgotten", online: false }), peerTwoRow({ online: true })];
      const parts = partitionRows([], membersReply({ rows }), ME);
      const all = [...parts.active, ...parts.pending, ...parts.unlisted, ...parts.excluded, ...parts.forgotten, ...parts.stale].map((r) => r.id_full);
      assert.equal(new Set(all).size, all.length, all.join(","));
    });

    it("stale keeps the order of the reply", () => {
      const a = staleRow("old-desktop", 777600);
      const b = staleRow("ghost-key", null);
      const parts = partitionRows([], membersReply({ rows: [selfRow(), b, a] }), ME);
      assert.deepEqual(ids(parts.stale), [b.id_full, a.id_full]);
    });
  });

  describe("a peer the reply does not know", () => {
    it("stays in the active list", () => {
      const peers = [peer(SELF_FULL, { self: true }), peer(PEER_FULL)];
      const parts = partitionRows(peers, membersReply({ rows: [selfRow()] }), ME);
      assert.deepEqual(ids(parts.active), [SELF_FULL, PEER_FULL]);
    });
  });
});
