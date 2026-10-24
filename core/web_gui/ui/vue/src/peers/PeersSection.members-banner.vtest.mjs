/* Which ban proposal the banner shows after "Not now" (written before the code: red until it exists): the whole page on a fake node.
 * "Not now" hides the banner of THAT proposal until the page is loaded again; a proposal about another member is then shown in its place.
 * A hidden proposal that disappears from the list (confirmed, withdrawn, the member left) is forgotten: if it comes back, it is shown again.
 * One proposal and its dialog: `PeersSection.members-vote.vtest.mjs`. Selectors and helpers: `testing/members-page.mjs`. */

import { afterEach, describe, expect, it } from "vitest";
import { PEER2_FULL, PEER_FULL, SELF_FULL } from "../testing/section.mjs";
import { excludedInfo, foreignVote, membersReply, peerOneRow, selfRow } from "../testing/members-fixtures.mjs";
import { ONE, TWO, openMembersPage, peerTwoNamed, stateWith } from "../testing/members-page.mjs";
import { unmountPage } from "../testing/page.mjs";

afterEach(unmountPage);

const AGAINST_ONE = "peer-two proposes banning peer-one from the network";
const AGAINST_TWO = "peer-one proposes banning peer-two from the network";

describe("two proposals wait for me", () => {
  it("the banner shows the first one in the order of the member list", async () => {
    const page = await twoProposalsPage();
    expect(page.text("#vote-title")).toBe(AGAINST_ONE);
  });

  describe("\"Not now\" on the first one", () => {
    it("shows the proposal about the other member in the banner", async () => {
      const page = await twoProposalsPage();
      await page.click("#vote-later");
      expect(page.has("#vote-banner")).toBe(true);
      expect(page.text("#vote-title")).toBe(AGAINST_TWO);
    });

    it("keeps the counter, the header link and the buttons of both rows", async () => {
      const page = await twoProposalsPage();
      await page.click("#vote-later");
      expect(page.text("#tab-peers .chip")).toBe("2");
      expect(page.has("#strip-vote")).toBe(true);
      expect(page.hasRowAction(ONE.code, "confirm")).toBe(true);
      expect(page.hasRowAction(TWO.code, "confirm")).toBe(true);
    });

    it("sends nothing", async () => {
      const page = await twoProposalsPage();
      await page.click("#vote-later");
      expect(page.node.calls.filter((c) => c.method === "POST")).toEqual([]);
    });

    it("the first one does not come back after polls", async () => {
      const page = await twoProposalsPage();
      await page.click("#vote-later");
      await page.waitPolls(2);
      expect(page.text("#vote-title")).toBe(AGAINST_TWO);
    });
  });

  describe("\"Not now\" on both", () => {
    it("no banner is left, the counter and the header link stay", async () => {
      const page = await twoProposalsPage();
      await page.click("#vote-later");
      await page.click("#vote-later");
      expect(page.has("#vote-banner")).toBe(false);
      expect(page.text("#tab-peers .chip")).toBe("2");
      expect(page.has("#strip-vote")).toBe(true);
    });

    it("the banner stays away after polls", async () => {
      const page = await twoProposalsPage();
      await page.click("#vote-later");
      await page.click("#vote-later");
      await page.waitPolls(2);
      expect(page.has("#vote-banner")).toBe(false);
    });
  });

  describe("\"Not now\" on the first, then I confirm the second", () => {
    it("the dialog is about the second one", async () => {
      const page = await twoProposalsPage();
      await page.click("#vote-later");
      await page.click("#vote-confirm");
      page.expectDialog({ title: "Confirm banning peer-two?", action: "Ban peer-two" });
    });

    it("afterwards the hidden proposal does not come back into the banner", async () => {
      const page = await twoProposalsPage();
      page.answerWith("exclude", membersReply({ rows: [selfRow(), againstOne(), excludedPeerTwo()] }));
      await page.click("#vote-later");
      await page.click("#vote-confirm");
      await page.confirmDialog();
      await page.waitPolls(1);
      expect(page.has("#vote-banner")).toBe(false);
      expect(page.text("#tab-peers .chip")).toBe("1");
    });
  });
});

describe("one proposal is hidden", () => {
  it("a proposal about another member that appears later is shown", async () => {
    const page = await openMembersPage({ members: members([againstOne(), peerTwoNamed()]), state: stateWith({ voting: 1 }) });
    await page.click("#vote-later");
    expect(page.has("#vote-banner")).toBe(false);
    page.members(members([againstOne(), againstTwo()]));
    await page.waitPolls(1);
    expect(page.text("#vote-title")).toBe(AGAINST_TWO);
  });

  it("a proposal that was withdrawn and made again is shown again", async () => {
    const page = await openMembersPage({ members: members([againstOne(), peerTwoNamed()]), state: stateWith({ voting: 1 }) });
    await page.click("#vote-later");
    page.members(members([peerOneRow(), peerTwoNamed()]));
    await page.waitPolls(1);
    expect(page.has("#vote-banner")).toBe(false);
    page.members(members([againstOne(), peerTwoNamed()]));
    await page.waitPolls(1);
    expect(page.text("#vote-title")).toBe(AGAINST_ONE);
  });

  it("the same proposal in the next replies stays hidden", async () => {
    const page = await openMembersPage({ members: members([againstOne(), peerTwoNamed()]), state: stateWith({ voting: 1 }) });
    await page.click("#vote-later");
    page.members(members([againstOne(), peerTwoNamed()]));
    await page.waitPolls(2);
    expect(page.has("#vote-banner")).toBe(false);
  });
});

// peer-two proposes banning peer-one, and peer-one proposes banning peer-two; I have voted for neither.
const againstOne = () => peerOneRow({ vote: foreignVote({ proposed_by: PEER2_FULL, voters: [PEER2_FULL] }) });
const againstTwo = () => peerTwoNamed({ vote: foreignVote() });
const members = (others) => membersReply({ rows: [selfRow(), ...others] });
const excludedPeerTwo = () =>
  peerTwoNamed({ status: "excluded", online: false, denied_by_me: "excluded", excluded: excludedInfo({ voters: [SELF_FULL, PEER_FULL] }) });

function twoProposalsPage() {
  return openMembersPage({ members: members([againstOne(), againstTwo()]), state: stateWith({ voting: 2 }) });
}
