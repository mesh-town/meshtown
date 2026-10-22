/* Blocking a member on this computer (written before the code: red until it exists): the whole page on a fake node.
 * The dialog, "Block peer-one" (the full id goes to `POST /api/members/block`), the blocked row, "Unblock" (no dialog), and what the
 * page does when an action is refused (409 / 422 / 500). Selectors and helpers: `PeersSection.members.vtest.mjs`, `testing/members-page.mjs`.
 * A blocked member stays in the network: only this computer refuses it. */

import { afterEach, describe, expect, it } from "vitest";
import { PEER_FULL } from "../testing/section.mjs";
import { membersReply, peerOneRow, selfRow } from "../testing/members-fixtures.mjs";
import { ME, ONE, openMembersPage, peerTwoNamed } from "../testing/members-page.mjs";
import { unmountPage } from "../testing/page.mjs";

afterEach(unmountPage);

describe("the dialog", () => {
  it("asks to block peer-one, says what changes, and puts the focus on Cancel", async () => {
    const page = await blockSetup();
    await page.openMenu(PEER_FULL);
    await page.pick("block");
    page.expectDialog({ title: "Block peer-one on this computer?", action: "Block peer-one" });
    expect(page.dialogBody()).toContain("peer-one won't be able to connect to this computer or open sites through it. Its open connections to you are cut.");
    expect(page.dialogBody()).toContain("Only this computer is affected: peer-one stays in the network and keeps working with other members.");
    expect(page.dialogBody()).toContain('To remove it for everyone, choose "Remove from network" instead.');
  });

  it("Cancel sends nothing, closes the dialog and returns the focus to the toggle", async () => {
    const page = await blockSetup();
    await page.openMenu(PEER_FULL);
    await page.pick("block");
    await page.cancelDialog();
    expect(page.isDialogOpen()).toBe(false);
    expect(page.postedBodies("block")).toEqual([]);
    expect(page.toggleHasFocus(PEER_FULL)).toBe(true);
  });

  it("Esc does the same as Cancel", async () => {
    const page = await blockSetup();
    await page.openMenu(PEER_FULL);
    await page.pick("block");
    await page.pressEscapeInDialog();
    expect(page.isDialogOpen()).toBe(false);
    expect(page.postedBodies("block")).toEqual([]);
  });

  it("says that the member is your exit when it is", async () => {
    const page = await blockSetup({ exits: [PEER_FULL] });
    await page.openMenu(PEER_FULL);
    await page.pick("block");
    expect(page.dialogBody()).toContain("peer-one is your exit: after this, sites won't open until you choose another exit.");
  });
});

describe("\"Block peer-one\"", () => {
  it("sends the full id to the block endpoint and closes the dialog", async () => {
    const page = await blockSetup();
    await blockPeerOne(page);
    expect(page.postedBodies("block")).toEqual([{ id: PEER_FULL }]);
    expect(page.isDialogOpen()).toBe(false);
  });

  it("marks the row: badge, what it cannot do, \"Unblock\" instead of \"Use as exit\"", async () => {
    const page = await blockSetup();
    await blockPeerOne(page);
    expect(page.badge(ONE.code)).toBe("Blocked on this computer");
    expect(page.lines(ONE.code)).toContain("Can't connect to you or open sites through you");
    expect(page.rowAction(ONE.code, "unblock").text()).toBe("Unblock");
    expect(page.rowAction(ONE.code, "unblock").attributes("aria-label")).toBe("Unblock peer-one");
    expect(page.peerRow(ONE.code).find("[data-exit-via]").exists()).toBe(false);
  });

  it("the other members keep their \"Use as exit\"", async () => {
    const page = await blockSetup();
    await blockPeerOne(page);
    expect(page.peerRow("cccc3333").find("[data-exit-via]").exists()).toBe(true);
  });

  it("tells under the list that it is blocked; peer-one stays in the list", async () => {
    const page = await blockSetup();
    await blockPeerOne(page);
    expect(page.status()).toBe("peer-one is blocked on this computer.");
    expect(page.listCodes()).toContain(ONE.code);
    expect(page.has("#removed-list")).toBe(false);
  });

  it("in the menu of that row \"Block on this computer…\" is disabled", async () => {
    const page = await blockSetup();
    await blockPeerOne(page);
    await page.openMenu(PEER_FULL);
    const block = page.wrapper.get('[role="menu"] [data-member-item="block"]');
    expect(block.attributes("aria-disabled")).toBe("true");
    expect(block.attributes("title")).toBe("Already blocked");
  });
});

describe("\"Unblock\"", () => {
  it("needs no dialog: sends the full id at once and the row is ordinary again", async () => {
    const page = await blockSetup();
    await blockPeerOne(page);
    page.answerWith("unblock", membersReply({ rows: [selfRow(), peerOneRow(), peerTwoNamed()] }));
    await page.clickRowAction(ONE.code, "unblock");
    expect(page.isDialogOpen()).toBe(false);
    expect(page.postedBodies("unblock")).toEqual([{ id: PEER_FULL }]);
    expect(page.badge(ONE.code)).toBeNull();
    expect(page.peerRow(ONE.code).find("[data-exit-via]").exists()).toBe(true);
  });
});

describe("an action is refused", () => {
  // NOTE: there is no text for failed actions; the test expects the message of the node under the list.
  it.each([
    [409, "busy", "The node is busy, try again"],
    [409, "not_member", "That node is not a member of the network"],
    [422, "invalid", "The id is not valid"],
    [500, "internal", "The block could not be saved"],
  ])("%i %s: the dialog is closed, the message of the node is under the list, the list stays", async (status, error, message) => {
    const page = await blockSetup();
    page.failWith("block", status, error, message);
    await blockPeerOne(page);
    expect(page.isDialogOpen()).toBe(false);
    expect(page.status()).toContain(message);
    expect(page.badge(ONE.code)).toBeNull();
    expect(page.has('button[data-member-menu="' + PEER_FULL + '"]')).toBe(true);
    expect(page.statusLine()).not.toContain("is not responding");
  });

  it("409 left: the same, the message of the node is shown", async () => {
    const page = await blockSetup();
    page.failWith("block", 409, "left", "This computer has left the network");
    await blockPeerOne(page);
    expect(page.status()).toContain("This computer has left the network");
    expect(page.statusLine()).not.toContain("is not responding");
  });

  it("the network fails: the page says so under the list and the dialog is closed", async () => {
    const page = await blockSetup();
    page.node.routes["POST /api/members/block"] = () => { throw new TypeError("Failed to fetch"); };
    await blockPeerOne(page);
    expect(page.isDialogOpen()).toBe(false);
    expect(page.status()).not.toBe("");
    expect(page.badge(ME.code)).toBeNull();
  });
});

// The three members; the block answer is "peer-one is blocked by me".
async function blockSetup({ exits = [] } = {}) {
  const page = await openMembersPage({ exits });
  const blocked = peerOneRow({ blocked_by_me: true, denied_by_me: "blocked" });
  page.answerWith("block", membersReply({ rows: [selfRow(), blocked, peerTwoNamed()] }));
  return page;
}

async function blockPeerOne(page) {
  await page.openMenu(PEER_FULL);
  await page.pick("block");
  await page.confirmDialog();
}
