/* Join requests on the "Members" tab: the card with requests, confirming through the dialog, the tab counter that adds requests
 * to bans, and the card of a computer that waits itself. The page runs on a fake node (`testing/members-page.mjs`). */

import { afterEach, describe, expect, it } from "vitest";
import { PEER2_FULL, PEER_FULL } from "../testing/section.mjs";
import { admissionInfo, foreignVote, membersReply, peerOneRow, pendingRow, selfRow } from "../testing/members-fixtures.mjs";
import { openMembersPage, peerTwoNamed, stateWith } from "../testing/members-page.mjs";
import { unmountPage } from "../testing/page.mjs";

afterEach(unmountPage);

const ID = "E".repeat(64);
const withPending = (over = {}) => membersReply({ rows: [selfRow(), peerOneRow(), pendingRow(over)] });

describe("a node waits to join", () => {
  it("is in the requests card, not in the member list; the tab counter is 1", async () => {
    const page = await openMembersPage({ members: withPending({ admission: admissionInfo({ label: "Anna's laptop" }) }), state: stateWith({ admitting: 1 }) });
    expect(page.text("#members-pending-list .peer-name")).toBe("Anna's laptop");
    expect(page.listCodes()).not.toContain("eeee5555");
    expect(page.text("#tab-peers .chip")).toBe("1");
    expect(page.attr("#tab-peers .chip", "aria-label")).toBe("1 request needs your confirmation");
  });

  it("Confirm opens the dialog; the answer posts admit and the row becomes a member", async () => {
    const page = await openMembersPage({ members: withPending(), state: stateWith({ admitting: 1 }) });
    const after = membersReply({ rows: [selfRow(), peerOneRow(), pendingRow({ status: "member", admission: null })] });
    page.answerWith("admit", after);
    await page.click('[data-pending="' + ID + '"] [data-member-action="admit"]');
    expect(page.isDialogOpen()).toBe(true);
    expect(page.text("#confirm-title")).toBe("Confirm eeee5555 joining?");
    expect(page.postedBodies("admit")).toEqual([]);
    await page.confirmDialog();
    expect(page.postedBodies("admit")).toEqual([{ id: ID }]);
    expect(page.status()).toBe("eeee5555 joined the network.");
    expect(page.has("#members-pending-list")).toBe(false);
  });

  it("Cancel in the dialog posts nothing", async () => {
    const page = await openMembersPage({ members: withPending() });
    await page.click('[data-pending="' + ID + '"] [data-member-action="admit"]');
    await page.cancelDialog();
    expect(page.postedBodies("admit")).toEqual([]);
  });

  it("a refusal shows the sentence of the node", async () => {
    const page = await openMembersPage({ members: withPending() });
    page.failWith("admit", 409, "not_pending", "This member isn't waiting for confirmation");
    await page.click('[data-pending="' + ID + '"] [data-member-action="admit"]');
    await page.confirmDialog();
    expect(page.status()).toBe("This member isn't waiting for confirmation");
  });

  it("after my confirmation: the line says so, no button, the menu withdraws", async () => {
    const mine = withPending({ admission: admissionInfo({ votes: 1, voters: [PEER_FULL], mine: true }) });
    const page = await openMembersPage({ members: mine });
    expect(page.has('[data-pending="' + ID + '"] [data-member-action="admit"]')).toBe(false);
    page.answerWith("unadmit", withPending());
    await page.openMenu(ID);
    await page.pick("unadmit");
    expect(page.postedBodies("unadmit")).toEqual([{ id: ID }]);
    expect(page.status()).toBe("Your confirmation for eeee5555 was withdrawn.");
  });
});

describe("the tab counter", () => {
  it("the tab counter adds join requests to bans", async () => {
    const rows = [selfRow(), peerOneRow({ vote: foreignVote({ proposed_by: PEER2_FULL, voters: [PEER2_FULL] }) }), peerTwoNamed(), pendingRow()];
    const page = await openMembersPage({ members: membersReply({ rows }), state: stateWith({ voting: 1, admitting: 1 }) });
    expect(page.text("#tab-peers .chip")).toBe("2");
    expect(page.attr("#tab-peers .chip", "aria-label")).toBe("2 requests need your confirmation");
    expect(page.text("#strip-vote")).toBe("2 requests to confirm");
  });

  it("a request I have already confirmed is not counted", async () => {
    const page = await openMembersPage({ members: withPending({ admission: admissionInfo({ mine: true }) }) });
    expect(page.has("#tab-peers .chip")).toBe(false);
  });
});

describe("this computer waits to join", () => {
  it("shows the card with the numbers from me_admission", async () => {
    const members = membersReply({ rows: [selfRow(), peerOneRow()], me_pending: true, me_admission: admissionInfo({ votes: 1, need: 2 }) });
    const page = await openMembersPage({ members });
    expect(page.text("#members-pending-card-text")).toBe("This computer is waiting for network members to confirm it (1 of 2).");
  });

  it("no card when the node is not pending", async () => {
    const page = await openMembersPage({});
    expect(page.has("#members-pending-card")).toBe(false);
  });
});
