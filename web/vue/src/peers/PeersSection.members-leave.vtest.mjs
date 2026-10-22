/* Leaving the network and being removed from it (written before the code: red until it exists): the whole page on a fake node.
 * Leaving: the dialog, `POST /api/members/leave {confirm:true}`, the page after it. Being out: the screen `#excluded-card`
 * (components: `members/ExcludedCard.vtest.mjs`) takes the place of the Members content and of the Home cards when
 * `me_excluded` is set in `/api/state.members` or in the member list (`/api/members` may even answer 404: the state is enough).
 * The variant of the card comes from my own row: `excluded.how === "left"` → "left", anything else → "removed".
 * Selectors and helpers: `PeersSection.members.vtest.mjs`, `testing/members-page.mjs`. */

import { afterEach, describe, expect, it } from "vitest";
import { PEER2_FULL, PEER_FULL, SELF_FULL } from "../testing/section.mjs";
import { excludedInfo, membersReply, peerOneRow, selfRow } from "../testing/members-fixtures.mjs";
import { ME, openMembersPage, peerTwoNamed, stateWith } from "../testing/members-page.mjs";
import { unmountPage } from "../testing/page.mjs";

afterEach(unmountPage);

describe("leaving the network", () => {
  describe("the dialog", () => {
    it("asks, says what happens, has no \"your exit\" paragraph even when it applies, and focuses Cancel", async () => {
      const page = await openMembersPage({ exits: [PEER_FULL] });
      await page.openMenu(SELF_FULL);
      await page.pick("leave");
      page.expectDialog({ title: 'Leave the network "demo-network"?', action: "Leave network" });
      expect(page.dialogBody()).toContain("This computer disconnects from all members, and sites stop opening through Mesh Town.");
      expect(page.dialogBody()).toContain("To come back, you'll need a new invite from a member.");
      expect(page.dialogBody()).not.toContain("is your exit");
    });

    it("Cancel sends nothing and returns the focus to the toggle of your row", async () => {
      const page = await openMembersPage();
      await page.openMenu(SELF_FULL);
      await page.pick("leave");
      await page.cancelDialog();
      expect(page.isDialogOpen()).toBe(false);
      expect(page.postedBodies("leave")).toEqual([]);
      expect(page.toggleHasFocus(SELF_FULL)).toBe(true);
    });
  });

  describe("\"Leave network\"", () => {
    it("sends the confirmation to the leave endpoint", async () => {
      const page = await leftPage();
      await leave(page);
      expect(page.postedBodies("leave")).toEqual([{ confirm: true }]);
      expect(page.isDialogOpen()).toBe(false);
    });

    it("the Members tab says that you left the network and how to come back", async () => {
      const page = await leftPage();
      await leave(page);
      expect(page.tabShown("peers")).toBe(true);
      expect(page.status()).toBe('You left the network "demo-network".');
      expect(page.text("#sec-peers")).toContain('You haven\'t joined a Mesh Town network yet. Paste an invite from a member on the "Home" tab.');
      expect(page.has("[data-member-menu]")).toBe(false);
    });

    it("the header says that the computer is not in a network", async () => {
      const page = await leftPage();
      await leave(page);
      expect(page.statusLine()).toContain("You haven't joined a Mesh Town network yet");
    });

    it("an answer \"changed: false\" (left already) looks the same", async () => {
      const page = await leftPage(false);
      await leave(page);
      expect(page.status()).toBe('You left the network "demo-network".');
    });
  });
});

describe("this computer was removed by the members", () => {
  it("the Members tab shows the card instead of the list, with the title and the text", async () => {
    const page = await openMembersPage({ members: removedMe(), state: stateWith({ me_excluded: true }) });
    expect(page.text("#excluded-card")).toContain('This computer was removed from the network "demo-network"');
    expect(page.text("#excluded-card")).toContain("Members of the network removed it. Sites no longer open through Mesh Town");
    expect(page.attr("#excluded-card", "role")).toBe("status");
    expect(page.has("#peers-list")).toBe(false);
    expect(page.has("[data-member-menu]")).toBe(false);
    expect(page.has("#vote-banner")).toBe(false);
  });

  it("the Home tab shows the same card", async () => {
    const page = await openMembersPage({ members: removedMe(), state: stateWith({ me_excluded: true }), tab: "home" });
    expect(page.text("#excluded-card")).toContain('This computer was removed from the network "demo-network"');
  });

  it("the header says it too", async () => {
    const page = await openMembersPage({ members: removedMe(), state: stateWith({ me_excluded: true }) });
    expect(page.statusLine()).toContain('Removed from the network "demo-network" — sites won\'t open through Mesh Town');
  });

  it("\"Join with another invite\" opens Home and focuses the invite field", async () => {
    const page = await openMembersPage({ members: removedMe(), state: stateWith({ me_excluded: true }) });
    await page.click("#excluded-join");
    expect(page.tabShown("home")).toBe(true);
    // NOTE: `#invite-field` does not exist in the app; the invite field of the Home tab is `#join-key-input`.
    expect(page.focusedId()).toBe("join-key-input");
  });

  it("`me_excluded` in the state alone is enough: the node does not know the list (404)", async () => {
    const page = await openMembersPage({ members: null, state: stateWith({ me_excluded: true }) });
    expect(page.has("#excluded-card")).toBe(true);
    expect(page.statusLine()).not.toContain("is not responding");
  });

  it("`me_excluded` in the list alone is enough, too", async () => {
    const page = await openMembersPage({ members: removedMe(), state: stateWith() });
    expect(page.has("#excluded-card")).toBe(true);
  });

  // NOTE: no `excluded` info on my own row (the reason is unknown): the card still shows, its wording is not asserted.
  it("an unknown reason: the card still shows", async () => {
    const page = await openMembersPage({ members: membersReply({ me_excluded: true }), state: stateWith({ me_excluded: true }) });
    expect(page.has("#excluded-card")).toBe(true);
  });
});

describe("this computer left on its own (the node still reports the network)", () => {
  it("the card has the \"left\" wording", async () => {
    const page = await openMembersPage({ members: leftMe(), state: stateWith({ me_excluded: true }) });
    expect(page.text("#excluded-card")).toContain('You left the network "demo-network"');
    expect(page.text("#excluded-card")).not.toContain("was removed from the network");
    expect(page.has("[data-member-menu]")).toBe(false);
  });
});

describe("somebody else left the network", () => {
  it("is in the group \"No longer in the network\" with the line \"left the network\"", async () => {
    const gone = peerOneRow({ status: "excluded", online: false, denied_by_me: "excluded", excluded: excludedInfo({ how: "left", voters: [] }) });
    const page = await openMembersPage({ members: membersReply({ rows: [selfRow(), gone, peerTwoNamed()] }) });
    expect(page.listCodes()).toEqual([ME.code, "cccc3333"]);
    expect(page.summaryOf("#removed-list")).toBe("No longer in the network (1)");
    expect(page.text("#removed-list li")).toMatch(/^peer-one · left the network on \d{1,2} Oct$/);
  });
});

// The leave answer: the node is out of the network now — no network in the state, no member list.
async function leftPage(changed = true) {
  const page = await openMembersPage();
  page.node.routes["POST /api/members/leave"] = () => {
    page.node.state = { ...stateWith(), network: null, peers: [] };
    page.node.routes["GET /api/members"] = { status: 409, body: { error: "busy", message: "The node is not in a network" } };
    return { ...membersReply(), changed };
  };
  return page;
}

async function leave(page) {
  await page.openMenu(SELF_FULL);
  await page.pick("leave");
  await page.confirmDialog();
  await page.waitPolls(1);
}

const mineExcluded = (how, voters) =>
  selfRow({ status: "excluded", online: false, denied_by_me: how === "left" ? "left" : "excluded", excluded: excludedInfo({ how, voters }) });

const removedMe = () => membersReply({ rows: [mineExcluded("votes", [PEER_FULL, PEER2_FULL]), peerOneRow(), peerTwoNamed()], me_excluded: true, need: 2 });
const leftMe = () => membersReply({ rows: [mineExcluded("left", []), peerOneRow(), peerTwoNamed()], me_excluded: true });

