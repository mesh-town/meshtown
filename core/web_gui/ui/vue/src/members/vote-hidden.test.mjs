/* `vote-hidden.ts`: which ban proposals the banner still has to show after "Not now". Pure functions.
 *
 * Contract fixed by these tests:
 *  - A proposal is told apart by `proposalKey(row)` = `row.id_full + ":" + row.vote.proposed_by`: the same member proposed
 *    for a ban by somebody else is another proposal.
 *  - `rows` is the list of rows that wait for my confirmation (`confirmable(...)`), in the order of the member list.
 *  - `pendingProposal(rows, hidden)`: the first row whose key is not in `hidden` (the very row object), or nothing (null) when
 *    every proposal is hidden or there are no rows. Neither argument is changed.
 *  - `pruneHidden(hidden, rows)`: a new list with the keys that still belong to a row of `rows`, in the old order; the keys
 *    of proposals that are gone (confirmed, withdrawn, the member left) are dropped, so a proposal that comes back is shown again. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { proposalKey, pendingProposal, pruneHidden } from "./vote-hidden.ts";
import { PEER2_FULL, PEER_FULL, SELF_FULL } from "../testing/section.mjs";
import { extraRow, foreignVote, peerOneRow, peerTwoRow } from "../testing/members-fixtures.mjs";

// peer-two proposes banning peer-one.
const againstOne = () => peerOneRow({ vote: foreignVote({ proposed_by: PEER2_FULL, voters: [PEER2_FULL] }) });
// peer-one proposes banning peer-two.
const againstTwo = () => peerTwoRow({ vote: foreignVote() });
const KEY_AGAINST_ONE = PEER_FULL + ":" + PEER2_FULL;
const KEY_AGAINST_TWO = PEER2_FULL + ":" + PEER_FULL;

describe("proposalKey", () => {
  it("is the id of the member to ban and the id of the proposer, joined by a colon", () => {
    assert.equal(proposalKey(againstOne()), KEY_AGAINST_ONE);
    assert.equal(proposalKey(againstTwo()), KEY_AGAINST_TWO);
  });

  it("is the same for the same proposal read again", () => {
    assert.equal(proposalKey(againstOne()), proposalKey(againstOne()));
  });

  it("differs for another member proposed by the same proposer", () => {
    const other = extraRow({ vote: foreignVote() });
    assert.notEqual(proposalKey(other), proposalKey(againstTwo()));
  });

  it("differs for the same member proposed by another proposer", () => {
    const bySomeoneElse = peerTwoRow({ vote: foreignVote({ proposed_by: SELF_FULL, voters: [SELF_FULL] }) });
    assert.notEqual(proposalKey(bySomeoneElse), proposalKey(againstTwo()));
  });
});

describe("pendingProposal", () => {
  describe("nothing is hidden", () => {
    it("is the first row", () => {
      const rows = [againstOne(), againstTwo()];
      assert.equal(pendingProposal(rows, []), rows[0]);
    });
  });

  describe("the first proposal is hidden", () => {
    it("is the next one", () => {
      const rows = [againstOne(), againstTwo()];
      assert.equal(pendingProposal(rows, [KEY_AGAINST_ONE]), rows[1]);
    });

    it("the hidden one in the middle is skipped, too", () => {
      const rows = [againstOne(), againstTwo(), extraRow({ vote: foreignVote() })];
      assert.equal(pendingProposal(rows, [KEY_AGAINST_ONE, KEY_AGAINST_TWO]), rows[2]);
    });
  });

  describe("every proposal is hidden", () => {
    it("there is nothing to show", () => {
      const rows = [againstOne(), againstTwo()];
      assert.ok(!pendingProposal(rows, [KEY_AGAINST_TWO, KEY_AGAINST_ONE]), "a hidden proposal came back");
    });
  });

  describe("there are no rows", () => {
    it("there is nothing to show", () => {
      assert.ok(!pendingProposal([], []));
      assert.ok(!pendingProposal([], [KEY_AGAINST_ONE]));
    });
  });

  describe("a hidden key belongs to another proposer of the same member", () => {
    it("does not hide this proposal", () => {
      const rows = [againstOne()];
      const hiddenOther = PEER_FULL + ":" + SELF_FULL;
      assert.equal(pendingProposal(rows, [hiddenOther]), rows[0]);
    });
  });

  it("changes neither the rows nor the hidden list", () => {
    const rows = Object.freeze([againstOne(), againstTwo()]);
    const hidden = Object.freeze([KEY_AGAINST_ONE]);
    assert.equal(pendingProposal(rows, hidden), rows[1]);
    assert.deepEqual([...hidden], [KEY_AGAINST_ONE]);
    assert.equal(rows.length, 2);
  });
});

describe("pruneHidden", () => {
  describe("every hidden proposal is still pending", () => {
    it("keeps all the keys in their order", () => {
      const rows = [againstOne(), againstTwo()];
      assert.deepEqual(pruneHidden([KEY_AGAINST_TWO, KEY_AGAINST_ONE], rows), [KEY_AGAINST_TWO, KEY_AGAINST_ONE]);
    });
  });

  describe("a hidden proposal is gone", () => {
    it("its key is dropped, the others stay", () => {
      assert.deepEqual(pruneHidden([KEY_AGAINST_ONE, KEY_AGAINST_TWO], [againstTwo()]), [KEY_AGAINST_TWO]);
    });

    it("the key of a proposal by another proposer of the same member does not keep it", () => {
      const sameMemberOtherProposer = peerOneRow({ vote: foreignVote({ proposed_by: SELF_FULL, voters: [SELF_FULL] }) });
      assert.deepEqual(pruneHidden([KEY_AGAINST_ONE], [sameMemberOtherProposer]), []);
    });

    it("with no rows at all the list becomes empty", () => {
      assert.deepEqual(pruneHidden([KEY_AGAINST_ONE, KEY_AGAINST_TWO], []), []);
    });

    it("a proposal that comes back after it was gone is shown again", () => {
      const hidden = pruneHidden([KEY_AGAINST_ONE], []);
      const rows = [againstOne()];
      assert.equal(pendingProposal(rows, hidden), rows[0]);
    });
  });

  describe("nothing is hidden", () => {
    it("stays empty", () => {
      assert.deepEqual(pruneHidden([], [againstOne()]), []);
      assert.deepEqual(pruneHidden([], []), []);
    });
  });

  it("changes neither the hidden list nor the rows", () => {
    const hidden = Object.freeze([KEY_AGAINST_ONE, KEY_AGAINST_TWO]);
    const rows = Object.freeze([againstTwo()]);
    assert.deepEqual(pruneHidden(hidden, rows), [KEY_AGAINST_TWO]);
    assert.deepEqual([...hidden], [KEY_AGAINST_ONE, KEY_AGAINST_TWO]);
  });
});
