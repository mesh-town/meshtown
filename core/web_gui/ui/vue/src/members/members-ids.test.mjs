/* `members-ids.ts`: which card an excluded computer sees, who the only other member is, who can be let in. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { canAdmit, excludedVariant, onlyOtherMember } from "./members-ids.ts";
import { PEER_FULL, SELF_FULL } from "../testing/section.mjs";
import { excludedInfo, membersReply, pendingRow, peerOneRow, selfRow, staleRow } from "../testing/members-fixtures.mjs";

describe("excludedVariant", () => {
  it("is banned when my row says I was removed, or the reply is missing", () => {
    const mine = selfRow({ status: "excluded", excluded: excludedInfo({ how: "banned" }) });
    assert.equal(excludedVariant(membersReply({ rows: [mine, peerOneRow()] }), SELF_FULL), "banned");
    assert.equal(excludedVariant(membersReply({ me_excluded: true }), SELF_FULL), "banned");
    assert.equal(excludedVariant(null, SELF_FULL), "banned");
  });

  it("left on my own stays left", () => {
    const mine = selfRow({ status: "excluded", excluded: excludedInfo({ how: "left" }) });
    assert.equal(excludedVariant(membersReply({ rows: [mine, peerOneRow()] }), SELF_FULL), "left");
  });
});

describe("onlyOtherMember", () => {
  const SUBJECT = staleRow("old-desktop", 777600);

  it("is the one member besides me and the subject", () => {
    const members = membersReply({ rows: [selfRow(), peerOneRow(), SUBJECT] });
    assert.equal(onlyOtherMember(members, SELF_FULL, SUBJECT.id_full), PEER_FULL);
  });

  it("is null with nobody but me and the subject, or with no list", () => {
    assert.equal(onlyOtherMember(membersReply({ rows: [selfRow(), SUBJECT] }), SELF_FULL, SUBJECT.id_full), null);
    assert.equal(onlyOtherMember(null, SELF_FULL, SUBJECT.id_full), null);
  });
});

describe("canAdmit", () => {
  it("is true for someone else's pending row", () => {
    assert.equal(canAdmit(pendingRow(), SELF_FULL), true);
  });

  it("is false for a member row, for my own row and for no row", () => {
    assert.equal(canAdmit(peerOneRow(), SELF_FULL), false);
    assert.equal(canAdmit(pendingRow({ id_full: SELF_FULL }), SELF_FULL), false);
    assert.equal(canAdmit(null, SELF_FULL), false);
  });
});
