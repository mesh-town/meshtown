/* Taking back my own ban proposal on the "Members" tab (written before the code: red until it exists): the whole page on a fake node.
 * The menu of a row where I proposed the ban has one item, "Withdraw proposal"; it sends `POST /api/members/unexclude {id}` at once,
 * without a dialog, and the row becomes an ordinary one again. A proposal of another member that I only confirmed keeps the disabled
 * "Waiting…" item. Selectors and helpers: `PeersSection.members.vtest.mjs`, `testing/members-page.mjs`. */

import { afterEach, describe, expect, it } from "vitest";
import { PEER2_FULL, PEER_FULL, SELF_FULL } from "../testing/section.mjs";
import { foreignVote, membersReply, peerOneRow, selfRow, voteInfo } from "../testing/members-fixtures.mjs";
import { ONE, TWO, openMembersPage, peerTwoNamed, threeMembers } from "../testing/members-page.mjs";
import { unmountPage } from "../testing/page.mjs";

afterEach(unmountPage);

const ORDINARY_MENU = ["Block on this computer…", "Ban from network…"];

describe("the menu of a row where I proposed the ban", () => {
  it("has one item, \"Withdraw proposal\", and it is enabled", async () => {
    const page = await pageWithMyProposal();
    await page.openMenu(PEER_FULL);
    expect(page.menuLabels()).toEqual(["Withdraw proposal"]);
    expect(page.wrapper.get('[data-member-item="withdraw"]').attributes("aria-disabled")).toBeUndefined();
  });

  it("the row says that I wait for another member, and has no \"Waiting…\" item", async () => {
    const page = await pageWithMyProposal();
    expect(page.badge(ONE.code)).toBe("Ban proposed");
    expect(page.lines(ONE.code)).toContain("You proposed banning it · waiting for 1 more member to confirm");
    await page.openMenu(PEER_FULL);
    expect(page.has('[data-member-item="waiting"]')).toBe(false);
  });

  it("the members I did not propose to ban keep the ordinary menu", async () => {
    const page = await pageWithMyProposal();
    await page.openMenu(PEER2_FULL);
    expect(page.menuLabels()).toEqual(ORDINARY_MENU);
  });
});

describe("the menu of a row where I only confirmed a proposal of another member", () => {
  it("has one disabled item: waiting for another member", async () => {
    const page = await pageWithProposalIConfirmed();
    await page.openMenu(PEER_FULL);
    expect(page.menuLabels()).toEqual(["Waiting for another member to confirm"]);
    expect(page.wrapper.get('[data-member-item="waiting"]').attributes("aria-disabled")).toBe("true");
  });

  it("has no \"Withdraw proposal\": the proposal is not mine", async () => {
    const page = await pageWithProposalIConfirmed();
    await page.openMenu(PEER_FULL);
    expect(page.has('[data-member-item="withdraw"]')).toBe(false);
  });
});

describe("\"Withdraw proposal\"", () => {
  describe("the request", () => {
    it("sends the full id to the unexclude endpoint", async () => {
      const page = await pageWithMyProposal();
      await withdraw(page);
      expect(page.postedBodies("unexclude")).toEqual([{ id: PEER_FULL }]);
    });

    it("asks nothing first: no dialog opens", async () => {
      const page = await pageWithMyProposal();
      await page.openMenu(PEER_FULL);
      await page.pick("withdraw");
      expect(page.isDialogOpen()).toBe(false);
      expect(page.postedBodies("unexclude")).toHaveLength(1);
    });

    it("does not touch the exclude endpoint", async () => {
      const page = await pageWithMyProposal();
      await withdraw(page);
      expect(page.postedBodies("exclude")).toEqual([]);
    });
  });

  describe("the row", () => {
    it("is an ordinary one again: no mark, no waiting line", async () => {
      const page = await pageWithMyProposal();
      await withdraw(page);
      expect(page.badge(ONE.code)).toBeNull();
      expect(page.lines(ONE.code)).toEqual([]);
    });

    it("stays in the list with \"Use as exit\" and the ordinary menu", async () => {
      const page = await pageWithMyProposal();
      await withdraw(page);
      expect(page.listCodes()).toContain(ONE.code);
      expect(page.peerRow(ONE.code).find("[data-exit-via]").exists()).toBe(true);
      await page.openMenu(PEER_FULL);
      expect(page.menuLabels()).toEqual(ORDINARY_MENU);
    });

    it("the other rows are not changed", async () => {
      const page = await pageWithMyProposal();
      await withdraw(page);
      expect(page.badge(TWO.code)).toBeNull();
      expect(page.listCodes()).toContain(TWO.code);
    });

    it("can be proposed for a ban again afterwards", async () => {
      const page = await pageWithMyProposal();
      await withdraw(page);
      page.answerWith("exclude", membersWithMyProposal());
      await page.openMenu(PEER_FULL);
      await page.pick("exclude");
      await page.confirmDialog();
      expect(page.postedBodies("exclude")).toEqual([{ id: PEER_FULL }]);
      expect(page.badge(ONE.code)).toBe("Ban proposed");
    });
  });

  describe("the line under the list", () => {
    it("says the proposal is withdrawn and that the member is no longer blocked on this computer", async () => {
      const page = await pageWithMyProposal();
      await withdraw(page);
      expect(page.status()).toBe("Ban proposal withdrawn. peer-one is no longer blocked on this computer.");
    });

    it("when I had also blocked the member myself, only the withdrawal is reported and the block stays", async () => {
      const page = await pageWithMyProposal();
      page.answerWith("unexclude", membersReply({ rows: [selfRow(), peerOneRow({ blocked_by_me: true, denied_by_me: "blocked" }), peerTwoNamed()] }));
      await withdraw(page);
      expect(page.status()).toBe("Ban proposal withdrawn.");
      expect(page.badge(ONE.code)).toBe("Blocked on this computer");
      expect(page.rowAction(ONE.code, "unblock").text()).toBe("Unblock");
    });
  });

  describe("the node refuses", () => {
    it("the message of the node is under the list and the proposal stays", async () => {
      const page = await pageWithMyProposal();
      page.failWith("unexclude", 409, "not_member", "That computer isn't in the member list");
      await withdraw(page);
      expect(page.status()).toContain("That computer isn't in the member list");
      expect(page.badge(ONE.code)).toBe("Ban proposed");
      await page.openMenu(PEER_FULL);
      expect(page.menuLabels()).toEqual(["Withdraw proposal"]);
    });
  });
});

// You proposed banning peer-one: one vote (yours) of the two needed.
const membersWithMyProposal = () =>
  membersReply({ rows: [selfRow(), peerOneRow({ vote: voteInfo(), denied_by_me: "voted_by_me" }), peerTwoNamed()] });

// The page opens with my proposal already in the list; the withdraw answer is the list without the vote.
async function pageWithMyProposal() {
  const page = await openMembersPage({ members: membersWithMyProposal() });
  page.answerWith("unexclude", threeMembers());
  return page;
}

// peer-two proposed banning peer-one and I confirmed it, the second confirmation has not come yet (a network of four or more).
async function pageWithProposalIConfirmed() {
  const vote = foreignVote({ proposed_by: PEER2_FULL, voters: [PEER2_FULL, SELF_FULL], mine: true, votes: 2, need: 3 });
  return openMembersPage({ members: membersReply({ rows: [selfRow(), peerOneRow({ vote }), peerTwoNamed()], need: 3 }) });
}

async function withdraw(page) {
  await page.openMenu(PEER_FULL);
  await page.pick("withdraw");
}
