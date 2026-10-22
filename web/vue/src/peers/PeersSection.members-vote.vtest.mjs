/* Removal votes on the "Members" tab (written before the code: red until it exists): the whole page on a fake node.
 * Proposing a removal, removing at once in a network of two, confirming somebody else's proposal (banner, tab counter, header link,
 * "Not now"), and the cases where a proposal is NOT shown to you (you are the target, you proposed it, you already voted).
 * Contract: `PeersSection.members.vtest.mjs` (selectors), `testing/members-page.mjs` (helpers), for who can confirm:
 * `canConfirm` = a `member` row with a vote that is not mine, not proposed by me, and not about me.
 * The time of a proposal ("on 19 Oct, 14:05") is not in the API and is not shown. */

import { afterEach, describe, expect, it } from "vitest";
import { PEER2_FULL, PEER_FULL, SELF_FULL } from "../testing/section.mjs";
import { excludedInfo, foreignVote, membersReply, peerOneRow, selfRow, voteInfo } from "../testing/members-fixtures.mjs";
import { ME, ONE, TWO, openMembersPage, peerTwoNamed, stateWith, threeMembers, twoMembers } from "../testing/members-page.mjs";
import { polls, unmountPage } from "../testing/page.mjs";

afterEach(unmountPage);

describe("proposing a removal in a network of three", () => {
  describe("the dialog", () => {
    it("asks to remove peer-one, explains the two-member rule and puts the focus on Cancel", async () => {
      const page = await proposeSetup();
      await page.openMenu(PEER_FULL);
      await page.pick("exclude");
      page.expectDialog({ title: "Remove peer-one from the network?", action: "Propose removal" });
      const body = page.dialogBody();
      expect(body).toContain('peer-one will lose access to the network "demo-network". This takes two members: you propose, and one more member confirms.');
      expect(body).toContain("On this computer, peer-one is blocked right away.");
      expect(body).toContain("For everyone else, it's removed after a second member confirms. Its open connections are then cut.");
      expect(body).toContain('To keep peer-one away from this computer only, choose "Block on this computer" instead.');
    });

    it("Cancel sends nothing, closes the dialog and returns the focus to the toggle", async () => {
      const page = await proposeSetup();
      await page.openMenu(PEER_FULL);
      await page.pick("exclude");
      await page.cancelDialog();
      expect(page.isDialogOpen()).toBe(false);
      expect(page.postedBodies("exclude")).toEqual([]);
      expect(page.toggleHasFocus(PEER_FULL)).toBe(true);
      expect(page.badge(ONE.code)).toBeNull();
    });

    it("Esc does the same as Cancel", async () => {
      const page = await proposeSetup();
      await page.openMenu(PEER_FULL);
      await page.pick("exclude");
      await page.pressEscapeInDialog();
      expect(page.isDialogOpen()).toBe(false);
      expect(page.postedBodies("exclude")).toEqual([]);
      expect(page.toggleHasFocus(PEER_FULL)).toBe(true);
    });

    it("says that the member is your exit when it is", async () => {
      const page = await proposeSetup({ exits: [PEER_FULL] });
      await page.openMenu(PEER_FULL);
      await page.pick("exclude");
      expect(page.dialogBody()).toContain("peer-one is your exit: after this, sites won't open until you choose another exit.");
    });

    it("does not say that when the member is not your exit", async () => {
      const page = await proposeSetup();
      await page.openMenu(PEER_FULL);
      await page.pick("exclude");
      expect(page.dialogBody()).not.toContain("is your exit");
    });
  });

  describe("\"Propose removal\"", () => {
    it("sends the full id of peer-one to the exclude endpoint and closes the dialog", async () => {
      const page = await proposeSetup();
      await page.openMenu(PEER_FULL);
      await page.pick("exclude");
      await page.confirmDialog();
      expect(page.postedBodies("exclude")).toEqual([{ id: PEER_FULL }]);
      expect(page.isDialogOpen()).toBe(false);
    });

    it("marks the row \"Removal proposed\" with the waiting line; the member stays in the list", async () => {
      const page = await proposeSetup();
      await proposeRemovalOfPeerOne(page);
      expect(page.badge(ONE.code)).toBe("Removal proposed");
      expect(page.lines(ONE.code)).toContain("You proposed removing it · waiting for 1 more member to confirm");
      expect(page.listCodes()).toContain(ONE.code);
      expect(page.has("#removed-list")).toBe(false);
    });

    it("tells under the list what happens next", async () => {
      const page = await proposeSetup();
      await proposeRemovalOfPeerOne(page);
      // NOTE: the text says "is already blocked on this computer" although the reply has blocked_by_me=false.
      expect(page.status()).toBe(
        "Removal proposed. peer-one is already blocked on this computer. Ask another member to open Mesh Town and confirm on the Members tab.",
      );
    });

    // Taking the proposal back: `PeersSection.members-withdraw.vtest.mjs`.
    it("leaves one item in the menu of that row: withdraw the proposal", async () => {
      const page = await proposeSetup();
      await proposeRemovalOfPeerOne(page);
      await page.openMenu(PEER_FULL);
      expect(page.menuLabels()).toEqual(["Withdraw proposal"]);
      expect(page.wrapper.get('[data-member-item="withdraw"]').attributes("aria-disabled")).toBeUndefined();
    });

    it("shows the proposer no banner, no counter on the tab and no link in the header", async () => {
      const page = await proposeSetup();
      await proposeRemovalOfPeerOne(page);
      expect(page.has("#vote-banner")).toBe(false);
      expect(page.has("#tab-peers .chip")).toBe(false);
      expect(page.has("#strip-vote")).toBe(false);
    });
  });
});

describe("removing a member in a network of two", () => {
  it("the dialog says it is removed right away, without the two-member rule", async () => {
    const page = await openMembersPage({ members: twoMembers() });
    await page.openMenu(PEER_FULL);
    await page.pick("exclude");
    page.expectDialog({ title: "Remove peer-one from the network?", action: "Remove peer-one" });
    expect(page.dialogBody()).toContain(
      "You and peer-one are the only members, so peer-one is removed right away and its open connections are cut. You can't undo this here.",
    );
    expect(page.dialogBody()).not.toContain("This takes two members");
  });

  it("the member leaves the list at once and lands in the group of those no longer in the network", async () => {
    const page = await openMembersPage({ members: twoMembers() });
    page.answerWith("exclude", membersReply({ rows: [selfRow(), removedPeerOne([SELF_FULL])], need: 1 }));
    await page.openMenu(PEER_FULL);
    await page.pick("exclude");
    await page.confirmDialog();
    expect(page.postedBodies("exclude")).toEqual([{ id: PEER_FULL }]);
    expect(page.listCodes()).toEqual([ME.code, TWO.code]); // peer-two is not in the member list: shown as a plain row
    expect(page.summaryOf("#removed-list")).toBe("No longer in the network (1)");
    expect(page.status()).toBe("peer-one was removed from the network.");
    expect(page.has(".peer-badge")).toBe(false);
  });
});

describe("another member proposed removing peer-one", () => {
  describe("what you see", () => {
    it("a banner that names who proposes removing whom", async () => {
      const page = await foreignProposalPage();
      expect(page.text("#vote-title")).toBe("peer-two proposes removing peer-one from the network");
      expect(page.text("#vote-banner")).toContain("If you confirm, peer-one is removed for everyone and its open connections are cut.");
      expect(page.attr("#vote-banner", "role")).toBe("region");
    });

    it("a counter 1 on the Members tab", async () => {
      const page = await foreignProposalPage();
      expect(page.text("#tab-peers .chip")).toBe("1");
      expect(page.attr("#tab-peers .chip", "aria-label")).toBe("1 request needs your confirmation");
      expect(page.attr("#tab-peers .chip", "title")).toBe("Members need your confirmation");
    });

    it("a link to the tab in the header line", async () => {
      const page = await foreignProposalPage();
      expect(page.text("#status-line #strip-vote")).toBe("1 request to confirm");
      expect(page.attr("#strip-vote", "href")).toBe("#peers");
    });

    it("the row: \"Removal proposed\", who proposed it and a \"Confirm removal…\" button", async () => {
      const page = await foreignProposalPage();
      expect(page.badge(ONE.code)).toBe("Removal proposed");
      expect(page.lines(ONE.code)).toContain("Proposed by peer-two · needs 1 more confirmation");
      const button = page.rowAction(ONE.code, "confirm");
      expect(button.text()).toBe("Confirm removal…");
      expect(button.attributes("aria-label")).toBe("Confirm removal: peer-one");
    });

    it("the confirm button of the row survives polling", async () => {
      const page = await foreignProposalPage();
      const button = page.rowAction(ONE.code, "confirm").element;
      await polls(page.node, 2);
      expect(button.isConnected).toBe(true);
      expect(page.rowAction(ONE.code, "confirm").element).toBe(button);
    });
  });

  describe("\"Confirm removal…\" in the banner", () => {
    it("asks again in a dialog that says whose proposal it is, with Cancel focused", async () => {
      const page = await foreignProposalPage();
      await page.click("#vote-confirm");
      page.expectDialog({ title: "Confirm removing peer-one?", action: "Remove peer-one" });
      const body = page.dialogBody();
      expect(body).toContain('peer-two proposed removing peer-one from the network "demo-network"');
      expect(body).toContain("Yours is the second confirmation: peer-one will be removed for everyone right away and its open connections will be cut.");
      expect(body).toContain("Confirm only if you know why. If unsure, ask peer-two first.");
      expect(page.postedBodies("exclude")).toEqual([]);
    });

    it("\"Remove peer-one\" sends the full id; the member leaves the list for the removed group", async () => {
      const page = await foreignProposalPage();
      page.answerWith("exclude", afterSecondVote());
      await page.click("#vote-confirm");
      await page.confirmDialog();
      expect(page.postedBodies("exclude")).toEqual([{ id: PEER_FULL }]);
      expect(page.listCodes().sort()).toEqual([ME.code, TWO.code].sort());
      expect(page.summaryOf("#removed-list")).toBe("No longer in the network (1)");
      expect(page.text("#removed-list li")).toMatch(/^peer-one · removed on \d{1,2} Oct · by .+ and .+$/);
      expect(page.status()).toBe("peer-one was removed from the network.");
    });

    it("afterwards the banner, the counter and the header link are gone", async () => {
      const page = await foreignProposalPage();
      page.answerWith("exclude", afterSecondVote());
      await page.click("#vote-confirm");
      await page.confirmDialog();
      expect(page.has("#vote-banner")).toBe(false);
      expect(page.has("#tab-peers .chip")).toBe(false);
      expect(page.has("#strip-vote")).toBe(false);
    });

    it("Cancel changes nothing: no request, the banner and the counter stay", async () => {
      const page = await foreignProposalPage();
      await page.click("#vote-confirm");
      await page.cancelDialog();
      expect(page.isDialogOpen()).toBe(false);
      expect(page.postedBodies("exclude")).toEqual([]);
      expect(page.has("#vote-banner")).toBe(true);
      expect(page.text("#tab-peers .chip")).toBe("1");
    });
  });

  describe("\"Confirm removal…\" on the row", () => {
    it("opens the same dialog", async () => {
      const page = await foreignProposalPage();
      await page.clickRowAction(ONE.code, "confirm");
      page.expectDialog({ title: "Confirm removing peer-one?", action: "Remove peer-one" });
    });
  });

  describe("\"Not now\"", () => {
    it("hides the banner, keeps the counter, the header link and the row button; sends nothing", async () => {
      const page = await foreignProposalPage();
      await page.click("#vote-later");
      expect(page.has("#vote-banner")).toBe(false);
      expect(page.text("#tab-peers .chip")).toBe("1");
      expect(page.has("#strip-vote")).toBe(true);
      expect(page.hasRowAction(ONE.code, "confirm")).toBe(true);
      expect(page.node.calls.filter((c) => c.method === "POST")).toEqual([]);
    });

    it("the banner stays hidden after a poll", async () => {
      const page = await foreignProposalPage();
      await page.click("#vote-later");
      await polls(page.node, 2);
      expect(page.has("#vote-banner")).toBe(false);
    });
  });
});

describe("a proposal you are not asked about", () => {
  it("the removal of YOU: no banner, no counter, no header link, no badge or button on your row", async () => {
    const rows = [selfRow({ vote: foreignVote() }), peerOneRow(), peerTwoNamed()];
    const page = await openMembersPage({ members: membersReply({ rows }), state: stateWith({ voting: 1 }) });
    expect(page.has("#vote-banner")).toBe(false);
    expect(page.has("#tab-peers .chip")).toBe(false);
    expect(page.has("#strip-vote")).toBe(false);
    expect(page.badge(ME.code)).toBeNull();
    expect(page.hasRowAction(ME.code, "confirm")).toBe(false);
  });

  it("a proposal you already voted for (not yours, but you are among the voters)", async () => {
    // The server marks it mine: that alone is enough not to ask again.
    const vote = foreignVote({ proposed_by: PEER2_FULL, voters: [PEER2_FULL], mine: true });
    const rows = [selfRow(), peerOneRow({ vote }), peerTwoNamed()];
    const page = await openMembersPage({ members: membersReply({ rows }), state: stateWith({ voting: 1 }) });
    expect(page.has("#vote-banner")).toBe(false);
    expect(page.has("#tab-peers .chip")).toBe(false);
    expect(page.has("#strip-vote")).toBe(false);
    expect(page.hasRowAction(ONE.code, "confirm")).toBe(false);
  });

  it("a proposal of peer-one against peer-two (neither of them is you) is shown with both names", async () => {
    const rows = [selfRow(), peerOneRow(), peerTwoNamed({ vote: foreignVote() })];
    const page = await openMembersPage({ members: membersReply({ rows }), state: stateWith({ voting: 1 }) });
    expect(page.text("#vote-title")).toBe("peer-one proposes removing peer-two from the network");
  });
});

describe("two proposals wait for you", () => {
  it("the counter says 2 and the header link says how many", async () => {
    const rows = [selfRow(), peerOneRow({ vote: foreignVote({ proposed_by: PEER2_FULL, voters: [PEER2_FULL] }) }), peerTwoNamed({ vote: foreignVote() })];
    const page = await openMembersPage({ members: membersReply({ rows }), state: stateWith({ voting: 2 }) });
    expect(page.text("#tab-peers .chip")).toBe("2");
    expect(page.attr("#tab-peers .chip", "aria-label")).toBe("2 requests need your confirmation");
    expect(page.text("#strip-vote")).toBe("2 requests to confirm");
  });
});

// The three members; `exits` — the exit members chosen in the config.
function proposeSetup({ exits = [] } = {}) {
  return openMembersPage({ members: threeMembers(), exits }).then((page) => {
    page.answerWith("exclude", proposedByMe());
    return page;
  });
}

async function proposeRemovalOfPeerOne(page) {
  await page.openMenu(PEER_FULL);
  await page.pick("exclude");
  await page.confirmDialog();
}

// You proposed removing peer-one: one vote (yours) of the two needed.
const proposedByMe = () => membersReply({ rows: [selfRow(), peerOneRow({ vote: voteInfo(), denied_by_me: "voted_by_me" }), peerTwoNamed()] });

// peer-two proposed removing peer-one, you have not voted.
function foreignProposalPage() {
  const vote = foreignVote({ proposed_by: PEER2_FULL, voters: [PEER2_FULL] });
  const members = membersReply({ rows: [selfRow(), peerOneRow({ vote }), peerTwoNamed()] });
  return openMembersPage({ members, state: stateWith({ voting: 1 }) }).then((page) => {
    page.answerWith("exclude", afterSecondVote());
    return page;
  });
}

// ...and after your confirmation it reached the two votes: peer-one is out.
const afterSecondVote = () => membersReply({ rows: [selfRow(), removedPeerOne([PEER2_FULL, SELF_FULL]), peerTwoNamed()] });

const removedPeerOne = (voters) => peerOneRow({ status: "excluded", online: false, denied_by_me: "excluded", excluded: excludedInfo({ voters }) });

