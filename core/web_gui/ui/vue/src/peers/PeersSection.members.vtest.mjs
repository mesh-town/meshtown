/* The "Members" tab with the member list (written before the code: red until it exists): the whole page on a fake node.
 * Here: an older node without the list, the rows and their menus, nodes that are not in the list, a failing list request.
 * The other flows: `PeersSection.members-vote.vtest.mjs` (propose / confirm a removal), `…-block.vtest.mjs` (block, errors, the dialog),
 * `…-leave.vtest.mjs` (leaving, the excluded screen).
 *
 * Contract of the page (see `testing/members-page.mjs` for the helpers):
 *  - the page asks `GET /api/members` after every accepted `/api/state` that has a network; the failure of that request never
 *    shows "not responding" (404 / 409 → no list at all; a network error → the previous list stays);
 *  - a row `#peers-list > li` gets the menu toggle `button[data-member-menu=<id_full>]`; the menu is
 *    `[role=menu] [role=menuitem][data-member-item=block|exclude|rename|leave|waiting|…]`;
 *  - the badge is `.peer-badge`, extra lines are `.peer-line`, the row's main action is `[data-member-action=confirm|unblock]`;
 *  - nodes with `status: "unlisted"` are not in `#peers-list` but in `details#unlisted-list`. */

import { afterEach, describe, expect, it } from "vitest";
import { PEER2_FULL, PEER_FULL, SELF_FULL } from "../testing/section.mjs";
import { EXTRA_FULL, extraRow, foreignVote, membersReply, peerOneRow, selfRow } from "../testing/members-fixtures.mjs";
import { ME, ONE, STRAY, TWO, openMembersPage, peerTwoNamed, stateWith } from "../testing/members-page.mjs";
import { unmountPage } from "../testing/page.mjs";

const NOT_RESPONDING = "is not responding";

afterEach(unmountPage);

describe("a node of an older version (no member list: GET /api/members answers 404)", () => {
  it("lists the members as before: the same rows and \"Use as exit\", no member menus, no badges", async () => {
    const page = await openMembersPage({ members: null });
    await page.waitPolls(2);
    expect(page.listCodes().sort()).toEqual([ME.code, ONE.code, TWO.code].sort());
    expect(page.has("[data-exit-via]")).toBe(true);
    expect(page.has("[data-member-menu]")).toBe(false);
    expect(page.has(".peer-badge")).toBe(false);
    expect(page.has("#vote-banner")).toBe(false);
    expect(page.has("#removed-list")).toBe(false);
    expect(page.has("#unlisted-list")).toBe(false);
  });

  it("the page does not claim that the node is not responding", async () => {
    const page = await openMembersPage({ members: null });
    await page.waitPolls(2);
    expect(page.askedMembers(), "the page does ask for the list").toBeGreaterThan(0);
    expect(page.statusLine()).not.toContain(NOT_RESPONDING);
    expect(page.has("#tab-peers .chip")).toBe(false);
    expect(page.has("#strip-vote")).toBe(false);
  });
});

describe("a node that is not in a network", () => {
  it("the page does not ask for the member list", async () => {
    const state = { ...stateWith(), network: null, peers: [] };
    const page = await openMembersPage({ state, tab: null });
    await page.waitPolls(2);
    expect(page.askedMembers()).toBe(0);
  });
});

describe("a node with the member list", () => {
  it("gives every row, yours too, a \"more actions\" toggle named after the member", async () => {
    const page = await openMembersPage();
    for (const [full, name] of [[SELF_FULL, "finch-unwary"], [PEER_FULL, "peer-one"], [PEER2_FULL, "peer-two"]]) {
      const toggle = page.wrapper.get('button[data-member-menu="' + full + '"]');
      expect(toggle.attributes("aria-label")).toBe("More actions for " + name);
      expect(toggle.attributes("aria-haspopup")).toBe("menu");
    }
  });

  it("the menu of another member: block on this computer, remove from the network", async () => {
    const page = await openMembersPage();
    await page.openMenu(PEER_FULL);
    expect(page.menuLabels()).toEqual(["Block on this computer…", "Remove from network…"]);
  });

  it("the menu of your own row: rename, leave the network", async () => {
    const page = await openMembersPage();
    await page.openMenu(SELF_FULL);
    expect(page.menuLabels()).toEqual(["Rename", "Leave network…"]);
  });

  it("an ordinary row has no badge and no removal text", async () => {
    const page = await openMembersPage();
    expect(page.badge(ONE.code)).toBeNull();
    expect(page.lines(ONE.code)).toEqual([]);
    expect(page.has("#vote-banner")).toBe(false);
    expect(page.has("#removed-list")).toBe(false);
  });

  it("the status line under the list is empty until something is done", async () => {
    const page = await openMembersPage();
    expect(page.status()).toBe("");
  });
});

describe("the toggles survive polling", () => {
  it("the toggle of a row is the same connected element after two polls", async () => {
    const page = await openMembersPage();
    const toggle = page.wrapper.get('button[data-member-menu="' + PEER_FULL + '"]').element;
    const mine = page.wrapper.get('button[data-member-menu="' + SELF_FULL + '"]').element;
    await page.waitPolls(2);
    expect(toggle.isConnected).toBe(true);
    expect(mine.isConnected).toBe(true);
    expect(page.wrapper.get('button[data-member-menu="' + PEER_FULL + '"]').element).toBe(toggle);
  });

  it("an open menu stays open after a poll", async () => {
    const page = await openMembersPage();
    await page.openMenu(PEER_FULL);
    await page.waitPolls(1);
    expect(page.menuLabels()).toEqual(["Block on this computer…", "Remove from network…"]);
  });

  it("a vote that appears in a later reply of the list shows on the row, the toggle stays", async () => {
    const page = await openMembersPage();
    const toggle = page.wrapper.get('button[data-member-menu="' + PEER_FULL + '"]').element;
    page.members(membersReply({ rows: [selfRow(), peerOneRow({ vote: foreignVote({ proposed_by: PEER2_FULL, voters: [PEER2_FULL] }) }), peerTwoNamed()] }));
    await page.waitPolls(1);
    expect(page.badge(ONE.code)).toBe("Removal proposed");
    expect(page.wrapper.get('button[data-member-menu="' + PEER_FULL + '"]').element).toBe(toggle);
  });
});

describe("a node that is not in the member list (status unlisted)", () => {
  it("is not in the list of members but in a group below it, counted in its summary", async () => {
    const page = await openMembersPage({ members: withStray() });
    expect(page.listCodes()).not.toContain(STRAY.code);
    expect(page.summaryOf("#unlisted-list")).toBe("Not in the member list (1)");
    expect(page.wrapper.get("#unlisted-list").text()).toContain(STRAY.name);
    expect(page.wrapper.get("#unlisted-list").text()).toContain(STRAY.code);
  });

  it("is marked \"Not a member\"", async () => {
    const page = await openMembersPage({ members: withStray() });
    expect(page.wrapper.get("#unlisted-list .peer-badge").text()).toBe("Not a member");
  });

  it("can only be blocked: no removal in its menu", async () => {
    const page = await openMembersPage({ members: withStray() });
    await page.openMenu(EXTRA_FULL);
    expect(page.menuLabels()).toEqual(["Block on this computer…"]);
  });

  it("blocking it goes through the same confirmation and sends its full id", async () => {
    const page = await openMembersPage({ members: withStray() });
    page.answerWith("block", withStray({ blocked_by_me: true, denied_by_me: "blocked" }));
    await page.openMenu(EXTRA_FULL);
    await page.pick("block");
    page.expectDialog({ title: "Block old-laptop on this computer?", action: "Block old-laptop" });
    await page.confirmDialog();
    expect(page.postedBodies("block")).toEqual([{ id: EXTRA_FULL }]);
    await page.openMenu(EXTRA_FULL);
    expect(page.menuLabels()).toEqual(["Unblock"]);
  });

  it("the group is absent when nobody is unlisted", async () => {
    const page = await openMembersPage();
    expect(page.has("#unlisted-list")).toBe(false);
  });
});

describe("a node of the network that is missing from the member list", () => {
  // `state.peers` knows peer-two, the reply of `/api/members` does not list it (yet).
  const withoutTwo = () => membersReply({ rows: [selfRow(), peerOneRow()] });

  it("is shown as an ordinary row among the others", async () => {
    const page = await openMembersPage({ members: withoutTwo() });
    expect(page.listCodes().sort()).toEqual([ME.code, ONE.code, TWO.code].sort());
  });

  it("has no \"more actions\" toggle, while the listed members keep theirs", async () => {
    const page = await openMembersPage({ members: withoutTwo() });
    expect(page.peerRow(TWO.code).find("[data-member-menu]").exists(), "a toggle on a row without a member entry").toBe(false);
    expect(page.has('button[data-member-menu="' + PEER_FULL + '"]')).toBe(true);
    expect(page.has('button[data-member-menu="' + SELF_FULL + '"]')).toBe(true);
  });

  it("has no badge, no extra line and no member action", async () => {
    const page = await openMembersPage({ members: withoutTwo() });
    expect(page.badge(TWO.code)).toBeNull();
    expect(page.lines(TWO.code)).toEqual([]);
    expect(page.hasRowAction(TWO.code, "confirm")).toBe(false);
    expect(page.hasRowAction(TWO.code, "unblock")).toBe(false);
  });

  it("is not put into a group under the list", async () => {
    const page = await openMembersPage({ members: withoutTwo() });
    expect(page.has("#unlisted-list")).toBe(false);
    expect(page.has("#removed-list")).toBe(false);
  });
});

describe("the list request fails", () => {
  it("409 busy: the member menus go away, the plain list stays, the header does not say \"not responding\"", async () => {
    const page = await openMembersPage();
    page.membersBusy();
    await page.waitPolls(1);
    expect(page.has("[data-member-menu]")).toBe(false);
    expect(page.listCodes().sort()).toEqual([ME.code, ONE.code, TWO.code].sort());
    expect(page.statusLine()).not.toContain(NOT_RESPONDING);
  });

  it("the network fails for this request only: the last list stays, the header does not say \"not responding\"", async () => {
    const page = await openMembersPage();
    page.membersUnreachable();
    await page.waitPolls(2);
    expect(page.has('button[data-member-menu="' + PEER_FULL + '"]')).toBe(true);
    expect(page.statusLine()).not.toContain(NOT_RESPONDING);
  });
});

// The three members plus a node that knocked and is not in the list.
function withStray(over = {}) {
  return membersReply({ rows: [selfRow(), peerOneRow(), peerTwoNamed(), extraRow({ status: "unlisted", denied_by_me: "unlisted", ...over })] });
}
