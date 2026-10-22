/* The reason of a removal on the "Members" tab (written before the code: red until it exists): the whole page on a fake node.
 * The dialog "Remove from network…" has an optional field; its text goes to `POST /api/members/exclude` as `reason` (an empty field
 * sends no `reason` at all). The banner and the confirmation dialog of another member's proposal show the reason the proposer gave.
 * Components: `members/ConfirmDialog.reason.vtest.mjs`, `members/VoteBanner.vtest.mjs`. Selectors and helpers:
 * `PeersSection.members.vtest.mjs`, `testing/members-page.mjs`. */

import { afterEach, describe, expect, it } from "vitest";
import { PEER2_FULL, PEER_FULL, SELF_FULL } from "../testing/section.mjs";
import { foreignVote, membersReply, peerOneRow, selfRow, voteInfo } from "../testing/members-fixtures.mjs";
import { ONE, openMembersPage, peerTwoNamed, stateWith, threeMembers, twoMembers } from "../testing/members-page.mjs";
import { unmountPage } from "../testing/page.mjs";

afterEach(unmountPage);

const REASON_LABEL = "Reason (optional, members will see it)";
const REASON_LINE = 'Reason from peer-two: "lost laptop"';

describe("the dialog \"Remove from network…\"", () => {
  it("asks for an optional reason in a labelled field of at most 80 characters; the focus stays on Cancel", async () => {
    const page = await openRemoveDialog();
    page.expectDialog({ title: "Remove peer-one from the network?", action: "Propose removal" });
    expect(page.text("label[for='member-reason']")).toBe(REASON_LABEL);
    expect(page.attr("input#member-reason", "maxlength")).toBe("80");
    expect(page.attr("input#member-reason", "type")).toBe("text");
  });

  it("the field is empty each time the dialog opens", async () => {
    const page = await openRemoveDialog();
    await typeReason(page, "lost laptop");
    await page.cancelDialog();
    await page.openMenu(PEER_FULL);
    await page.pick("exclude");
    expect(reasonValue(page)).toBe("");
  });

  it("a network of two removes at once: its dialog has no field", async () => {
    const page = await openMembersPage({ members: twoMembers() });
    await page.openMenu(PEER_FULL);
    await page.pick("exclude");
    expect(page.has("#member-reason")).toBe(false);
  });

  it("the block dialog has no field", async () => {
    const page = await openMembersPage();
    await page.openMenu(PEER_FULL);
    await page.pick("block");
    expect(page.has("#member-reason")).toBe(false);
  });

  it("the leave dialog has no field", async () => {
    const page = await openMembersPage();
    await page.openMenu(SELF_FULL);
    await page.pick("leave");
    expect(page.has("#member-reason")).toBe(false);
  });
});

describe("\"Propose removal\"", () => {
  it("sends the typed reason with the full id", async () => {
    const page = await openRemoveDialog();
    await typeReason(page, "lost laptop");
    await page.confirmDialog();
    expect(page.postedBodies("exclude")).toEqual([{ id: PEER_FULL, reason: "lost laptop" }]);
  });

  it("sends the reason without the spaces around it", async () => {
    const page = await openRemoveDialog();
    await typeReason(page, "  lost laptop  ");
    await page.confirmDialog();
    expect(page.postedBodies("exclude")).toEqual([{ id: PEER_FULL, reason: "lost laptop" }]);
  });

  it("an empty field sends only the id", async () => {
    const page = await openRemoveDialog();
    await page.confirmDialog();
    expect(page.postedBodies("exclude")).toEqual([{ id: PEER_FULL }]);
    expect("reason" in page.postedBodies("exclude")[0], "an empty reason was sent").toBe(false);
  });

  it("a field with only spaces sends only the id", async () => {
    const page = await openRemoveDialog();
    await typeReason(page, "     ");
    await page.confirmDialog();
    expect(page.postedBodies("exclude")).toEqual([{ id: PEER_FULL }]);
  });

  it("Enter in the field sends nothing and keeps the dialog open", async () => {
    const page = await openRemoveDialog();
    await typeReason(page, "lost laptop");
    await page.wrapper.get("#member-reason").trigger("keydown", { key: "Enter" });
    expect(page.postedBodies("exclude")).toEqual([]);
    expect(page.isDialogOpen()).toBe(true);
  });

  it("Cancel sends nothing, even with text in the field", async () => {
    const page = await openRemoveDialog();
    await typeReason(page, "lost laptop");
    await page.cancelDialog();
    expect(page.postedBodies("exclude")).toEqual([]);
    expect(page.isDialogOpen()).toBe(false);
  });

  it("the next request does not reuse the reason of the previous one", async () => {
    const page = await openRemoveDialog();
    await typeReason(page, "lost laptop");
    await page.confirmDialog();
    await page.openMenu(PEER2_FULL);
    await page.pick("exclude");
    expect(reasonValue(page)).toBe("");
    await page.confirmDialog();
    expect(page.postedBodies("exclude")).toEqual([{ id: PEER_FULL, reason: "lost laptop" }, { id: PEER2_FULL }]);
  });
});

describe("another member proposed removing peer-one and gave a reason", () => {
  it("the banner shows the reason of the proposer", async () => {
    const page = await foreignProposalPage("lost laptop");
    expect(page.text("#vote-banner .vote-reason")).toBe(REASON_LINE);
  });

  it("the banner shows markup in the reason as text", async () => {
    const page = await foreignProposalPage("<b>lost</b> laptop");
    expect(page.text("#vote-banner .vote-reason")).toBe('Reason from peer-two: "<b>lost</b> laptop"');
    expect(page.has("#vote-banner b")).toBe(false);
  });

  it("the dialog of \"Confirm removal…\" in the banner shows the same line", async () => {
    const page = await foreignProposalPage("lost laptop");
    await page.click("#vote-confirm");
    page.expectDialog({ title: "Confirm removing peer-one?", action: "Remove peer-one" });
    expect(page.dialogBody()).toContain(REASON_LINE);
  });

  it("the dialog of \"Confirm removal…\" on the row shows it too", async () => {
    const page = await foreignProposalPage("lost laptop");
    await page.clickRowAction(ONE.code, "confirm");
    expect(page.dialogBody()).toContain(REASON_LINE);
  });

  it("in the dialog the reason follows the explanation and comes before \"is your exit\"", async () => {
    const page = await foreignProposalPage("lost laptop", { exits: [PEER_FULL] });
    await page.click("#vote-confirm");
    const body = page.dialogBody();
    const at = (text) => body.indexOf(text);
    expect(at("Yours is the second confirmation")).toBeGreaterThanOrEqual(0);
    expect(at(REASON_LINE)).toBeGreaterThan(at("Yours is the second confirmation"));
    expect(at("peer-one is your exit")).toBeGreaterThan(at(REASON_LINE));
  });

  it("the confirmation dialog has no reason field of its own, and \"Remove peer-one\" sends only the id", async () => {
    const page = await foreignProposalPage("lost laptop");
    await page.click("#vote-confirm");
    expect(page.has("#member-reason")).toBe(false);
    await page.confirmDialog();
    expect(page.postedBodies("exclude")).toEqual([{ id: PEER_FULL }]);
  });
});

describe("another member proposed removing peer-one and gave no reason", () => {
  it("the banner has no reason line", async () => {
    const page = await foreignProposalPage(null);
    expect(page.has("#vote-banner")).toBe(true);
    expect(page.has("#vote-banner .vote-reason")).toBe(false);
  });

  it("the dialog has no reason line", async () => {
    const page = await foreignProposalPage(null);
    await page.click("#vote-confirm");
    expect(page.dialogBody()).not.toContain("Reason from");
  });
});

describe("my own proposal that has a reason", () => {
  it("shows the proposer neither a banner nor a reason line", async () => {
    const vote = voteInfo({ reason: "lost laptop" });
    const members = membersReply({ rows: [selfRow(), peerOneRow({ vote, denied_by_me: "voted_by_me" }), peerTwoNamed()] });
    const page = await openMembersPage({ members });
    expect(page.has("#vote-banner")).toBe(false);
    expect(page.has(".vote-reason")).toBe(false);
  });
});

// The three members; the exclude answer is "you proposed removing peer-one", whatever the request was.
async function openRemoveDialog() {
  const page = await openMembersPage({ members: threeMembers() });
  page.answerWith("exclude", membersReply({ rows: [selfRow(), peerOneRow({ vote: voteInfo(), denied_by_me: "voted_by_me" }), peerTwoNamed()] }));
  await page.openMenu(PEER_FULL);
  await page.pick("exclude");
  return page;
}

const typeReason = (page, text) => page.wrapper.get("#member-reason").setValue(text);
const reasonValue = (page) => page.wrapper.get("#member-reason").element.value;

// peer-two proposed removing peer-one with `reason`, I have not voted.
function foreignProposalPage(reason, { exits = [] } = {}) {
  const vote = foreignVote({ proposed_by: PEER2_FULL, voters: [PEER2_FULL], reason });
  const members = membersReply({ rows: [selfRow(), peerOneRow({ vote }), peerTwoNamed()] });
  return openMembersPage({ members, state: stateWith({ voting: 1 }), exits }).then((page) => {
    page.answerWith("exclude", membersReply({ rows: [selfRow(), peerOneRow({ status: "excluded", online: false }), peerTwoNamed()] }));
    return page;
  });
}
