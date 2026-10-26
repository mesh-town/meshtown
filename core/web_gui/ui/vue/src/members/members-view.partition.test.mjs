/* `members-view.ts`: `partitionRows`. The contract is in `members-view.test.mjs`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { partitionRows } from "./members-view.ts";
import { PEER2_FULL, PEER_FULL, SELF_FULL } from "../testing/section.mjs";
import { EXTRA_FULL, excludedInfo, extraRow, membersReply, pendingRow, peerOneRow, peerTwoRow, selfRow } from "../testing/members-fixtures.mjs";

const peer = (idFull, over = {}) => ({ id: idFull.slice(0, 8).toLowerCase(), id_full: idFull, name: null, online: true, ...over });
const ids = (rows) => rows.map((r) => r.id_full);

describe("partitionRows", () => {
  describe("members are not known (an old node or a failed request)", () => {
    it("every peer stays in the active list as it is", () => {
      const peers = [peer(PEER_FULL), peer(PEER2_FULL)];
      const parts = partitionRows(peers, null);
      assert.deepEqual(parts.active, peers);
      assert.equal(parts.active[0], peers[0], "the very same peer objects");
      assert.deepEqual(parts.unlisted, []);
      assert.deepEqual(parts.excluded, []);
    });

    it("no peers either: empty lists", () => {
      assert.deepEqual(partitionRows(null, null), { active: [], pending: [], unlisted: [], excluded: [] });
    });
  });

  describe("everybody is a member", () => {
    it("the active list is the peers in their order", () => {
      const peers = [peer(PEER2_FULL), peer(SELF_FULL, { self: true }), peer(PEER_FULL)];
      const reply = membersReply({ rows: [selfRow(), peerOneRow(), peerTwoRow()] });
      const parts = partitionRows(peers, reply);
      assert.deepEqual(ids(parts.active), [PEER2_FULL, SELF_FULL, PEER_FULL]);
      assert.equal(parts.unlisted.length + parts.excluded.length, 0);
    });
  });

  describe("a member was excluded", () => {
    it("its peer row leaves the active list and the row of the reply goes to excluded", () => {
      const peers = [peer(SELF_FULL, { self: true }), peer(PEER_FULL)];
      const gone = peerOneRow({ status: "excluded", excluded: excludedInfo() });
      const parts = partitionRows(peers, membersReply({ rows: [selfRow(), gone] }));
      assert.deepEqual(ids(parts.active), [SELF_FULL]);
      assert.deepEqual(parts.excluded, [gone]);
    });

    it("an excluded member that is not among the peers is still in excluded", () => {
      const gone = extraRow({ status: "excluded", excluded: excludedInfo({ how: "left", voters: [EXTRA_FULL] }) });
      const parts = partitionRows([peer(SELF_FULL, { self: true })], membersReply({ rows: [selfRow(), gone] }));
      assert.deepEqual(ids(parts.active), [SELF_FULL]);
      assert.deepEqual(parts.excluded, [gone]);
    });

    it("excluded keeps the order of the reply", () => {
      const a = extraRow({ status: "excluded", excluded: excludedInfo() });
      const b = peerTwoRow({ status: "excluded", excluded: excludedInfo() });
      const parts = partitionRows([], membersReply({ rows: [selfRow(), b, a] }));
      assert.deepEqual(ids(parts.excluded), [PEER2_FULL, EXTRA_FULL]);
    });
  });

  describe("a node is not in the member list", () => {
    it("its row of the reply goes to unlisted and its peer row leaves the active list", () => {
      const peers = [peer(SELF_FULL, { self: true }), peer(PEER2_FULL)];
      const stray = peerTwoRow({ status: "unlisted" });
      const parts = partitionRows(peers, membersReply({ rows: [selfRow(), stray] }));
      assert.deepEqual(ids(parts.active), [SELF_FULL]);
      assert.deepEqual(parts.unlisted, [stray]);
    });

    it("also when the node is not among the peers; the order of the reply is kept", () => {
      const a = extraRow({ status: "unlisted", online: false });
      const b = peerTwoRow({ status: "unlisted" });
      const parts = partitionRows([], membersReply({ rows: [selfRow(), b, a] }));
      assert.deepEqual(ids(parts.unlisted), [PEER2_FULL, EXTRA_FULL]);
    });
  });

  describe("a member the peer list does not have", () => {
    it("a minimal row after the peers in the active list", () => {
      const peers = [peer(SELF_FULL, { self: true })];
      const parts = partitionRows(peers, membersReply({ rows: [selfRow(), extraRow()] }));
      assert.deepEqual(ids(parts.active), [SELF_FULL, EXTRA_FULL]);
      const minimal = parts.active[1];
      assert.equal(minimal.id, "dddd4444");
      assert.equal(minimal.name, "old-laptop");
      assert.equal(minimal.online, true);
      assert.equal(!!minimal.self, false);
    });
  });



  describe("a member has been offline for a long time", () => {
    it.each([[604801], [9999999], [null]])("last seen %j: it stays in the active list", (lastSeen) => {
      const peers = [peer(SELF_FULL, { self: true }), peer(PEER_FULL, { online: false })];
      const reply = membersReply({ rows: [selfRow(), peerOneRow({ online: false, last_seen_sec: lastSeen })] });
      const parts = partitionRows(peers, reply);
      assert.deepEqual(ids(parts.active), [SELF_FULL, PEER_FULL]);
      assert.deepEqual(Object.keys(parts).sort(), ["active", "excluded", "pending", "unlisted"]);
    });

    it("a listed member the peer list does not have joins the active list", () => {
      const reply = membersReply({ rows: [selfRow(), peerTwoRow({ online: false, last_seen_sec: 9999999 })] });
      assert.deepEqual(ids(partitionRows([peer(SELF_FULL, { self: true })], reply).active), [SELF_FULL, PEER2_FULL]);
    });
  });

  describe("a node waits to join", () => {
    it("its row goes to pending", () => {
      const row = pendingRow();
      const parts = partitionRows([peer(SELF_FULL, { self: true })], membersReply({ rows: [selfRow(), row] }));
      assert.deepEqual(parts.pending, [row]);
      assert.deepEqual(ids(parts.active), [SELF_FULL]);
    });
  });

  describe("a peer the reply does not know", () => {
    it("stays in the active list", () => {
      const peers = [peer(SELF_FULL, { self: true }), peer(PEER_FULL)];
      const parts = partitionRows(peers, membersReply({ rows: [selfRow()] }));
      assert.deepEqual(ids(parts.active), [SELF_FULL, PEER_FULL]);
    });
  });
});
