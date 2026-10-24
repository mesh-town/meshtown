/* `members-ids.ts`: the removal-related lookups — which card a removed computer sees, who the only other member is,
 * and whether a pending ban is at risk when a member is removed. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { banAtRisk, excludedVariant, isMeRemoved, onlyOtherMember } from "./members-ids.ts";
import { PEER_FULL, SELF_FULL } from "../testing/section.mjs";
import { excludedInfo, foreignVote, membersReply, peerOneRow, selfRow, staleRow } from "../testing/members-fixtures.mjs";

describe("excludedVariant", () => {
  describe("this computer was removed from the member list", () => {
    it("variant_forgotten_when_me_removed", () => {
      const members = membersReply({ me_removed: true });
      assert.equal(excludedVariant(members, SELF_FULL), "forgotten");
    });

    it("isMeRemoved reads the flag of the reply", () => {
      assert.equal(isMeRemoved(membersReply({ me_removed: true })), true);
      assert.equal(isMeRemoved(membersReply()), false);
    });

    it("isMeRemoved is false while the list is not loaded", () => {
      assert.equal(isMeRemoved(null), false);
      assert.equal(isMeRemoved(undefined), false);
    });
  });

  describe("this computer was also banned", () => {
    it("ban_wins_over_removed", () => {
      const members = membersReply({ me_removed: true, me_excluded: true });
      assert.equal(excludedVariant(members, SELF_FULL), "banned");
    });

    it("ban_wins_over_removed: my own row says it was banned by votes", () => {
      const mine = selfRow({ status: "excluded", excluded: excludedInfo({ how: "votes" }) });
      const members = membersReply({ rows: [mine, peerOneRow()], me_removed: true });
      assert.equal(excludedVariant(members, SELF_FULL), "banned");
    });
  });

  describe("neither flag is set, or the reply is missing", () => {
    it("is banned, as before the removal existed", () => {
      assert.equal(excludedVariant(membersReply(), SELF_FULL), "banned");
      assert.equal(excludedVariant(null, SELF_FULL), "banned");
    });

    it("left on my own stays left", () => {
      const mine = selfRow({ status: "excluded", excluded: excludedInfo({ how: "left" }) });
      assert.equal(excludedVariant(membersReply({ rows: [mine, peerOneRow()] }), SELF_FULL), "left");
    });
  });
});

describe("onlyOtherMember", () => {
  const SUBJECT = staleRow("old-desktop", 777600);

  it("only_other_member_for_two", () => {
    const members = membersReply({ rows: [selfRow(), peerOneRow(), SUBJECT], need: 1 });
    assert.equal(onlyOtherMember(members, SELF_FULL, SUBJECT.id_full), PEER_FULL);
  });

  it("the subject is never the answer", () => {
    const members = membersReply({ rows: [selfRow(), SUBJECT, peerOneRow()], need: 1 });
    assert.notEqual(onlyOtherMember(members, SELF_FULL, SUBJECT.id_full), SUBJECT.id_full);
  });

  // QUESTION: the contract does not say what a list with no second member gives; null follows the signature `string | null`.
  it("nobody but me and the subject: null", () => {
    const members = membersReply({ rows: [selfRow(), SUBJECT], need: 1 });
    assert.equal(onlyOtherMember(members, SELF_FULL, SUBJECT.id_full), null);
  });

  it("no list yet: null", () => {
    assert.equal(onlyOtherMember(null, SELF_FULL, SUBJECT.id_full), null);
  });
});

describe("banAtRisk", () => {
  const SUBJECT = staleRow("old-desktop", 777600);
  const pendingBan = () => peerOneRow({ vote: foreignVote() });

  it("a ban is pending and two members would be left", () => {
    const members = membersReply({ rows: [selfRow(), SUBJECT, pendingBan()] });
    assert.equal(banAtRisk(members, SUBJECT.id_full), true);
  });

  it("three members would be left: the threshold does not drop", () => {
    const members = membersReply({ rows: [selfRow(), SUBJECT, pendingBan(), staleRow("spare", 10)] });
    assert.equal(banAtRisk(members, SUBJECT.id_full), false);
  });

  it("no ban is pending", () => {
    const members = membersReply({ rows: [selfRow(), SUBJECT, peerOneRow()] });
    assert.equal(banAtRisk(members, SUBJECT.id_full), false);
  });

  it("the only pending ban is on the subject itself", () => {
    const withVote = { ...SUBJECT, vote: foreignVote() };
    const members = membersReply({ rows: [selfRow(), withVote, peerOneRow()] });
    assert.equal(banAtRisk(members, SUBJECT.id_full), false);
  });

  it("only rows with the status member are counted", () => {
    const gone = peerOneRow({ id_full: "F".repeat(64), status: "excluded", excluded: excludedInfo() });
    const members = membersReply({ rows: [selfRow(), SUBJECT, pendingBan(), gone] });
    assert.equal(banAtRisk(members, SUBJECT.id_full), true);
  });

  it("no list yet", () => {
    assert.equal(banAtRisk(null, SUBJECT.id_full), false);
  });
});
