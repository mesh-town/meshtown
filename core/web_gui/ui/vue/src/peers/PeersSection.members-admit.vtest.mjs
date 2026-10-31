/* Join requests on the "Members" tab: the card with requests, confirming through the dialog, the tab counter of requests. The page runs on a fake node (`testing/members-page.mjs`). */

import { afterEach, describe, expect, it } from "vitest";
import { admissionInfo, membersReply, peerOneRow, pendingRow, selfRow } from "../testing/members-fixtures.mjs";
import { openMembersPage, stateWith } from "../testing/members-page.mjs";
import { unmountPage } from "../testing/page.mjs";

afterEach(unmountPage);

const ID = "E".repeat(64);
const withPending = (over = {}) => membersReply({ rows: [selfRow(), peerOneRow(), pendingRow(over)], me_admin: true });

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
    page.failWith("admit", 409, "not_pending", "This member isn't waiting for admission");
    await page.click('[data-pending="' + ID + '"] [data-member-action="admit"]');
    await page.confirmDialog();
    expect(page.status()).toBe("This member isn't waiting for admission");
  });
});

describe("the tab counter", () => {
  it("counts join requests for an admin", async () => {
    const rows = [selfRow(), peerOneRow(), pendingRow(), pendingRow({ id_full: "F".repeat(64), id: "ffff6666" })];
    const page = await openMembersPage({ members: membersReply({ rows, me_admin: true }), state: stateWith({ admitting: 2 }) });
    expect(page.text("#tab-peers .chip")).toBe("2");
    expect(page.attr("#tab-peers .chip", "aria-label")).toBe("2 requests need your confirmation");
    expect(page.has("#strip-vote")).toBe(true);
  });

  it("shows no counter to a member who is not an admin", async () => {
    const page = await openMembersPage({ members: membersReply({ rows: [selfRow(), peerOneRow(), pendingRow()], me_admin: false }) });
    expect(page.has("#tab-peers .chip")).toBe(false);
  });
});
