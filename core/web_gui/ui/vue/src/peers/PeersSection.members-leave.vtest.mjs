/* Leaving the network and being banned from it (written before the code: red until it exists): the whole page on a fake node.
 * Leaving: the dialog, `POST /api/members/leave {confirm:true}`, the page after it. Being out: the screen `#excluded-card`
 * (components: `members/ExcludedCard.vtest.mjs`) takes the place of the Members content and of the Home cards when
 * `me_excluded` is set in `/api/state.members` or in the member list (`/api/members` may even answer 404: the state is enough).
 * The variant of the card comes from my own row: `excluded.how === "left"` → "left", anything else → "banned".
 * Selectors and helpers: `PeersSection.members.vtest.mjs`, `testing/members-page.mjs`. */

import { afterEach, describe, expect, it } from "vitest";
import { PEER2_FULL, PEER_FULL, SELF_FULL } from "../testing/section.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { EXTRA_FULL, excludedInfo, extraRow, membersReply, peerOneRow, selfRow } from "../testing/members-fixtures.mjs";
import { ME, openMembersPage, peerTwoNamed, stateWith } from "../testing/members-page.mjs";
import { NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, settle, unmountPage } from "../testing/page.mjs";
import { baseConfig, baseState } from "../testing/section.mjs";

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

describe("this computer was removed by an admin", () => {
  it("the Members tab shows the card instead of the list, with the title and the text", async () => {
    const page = await openMembersPage({ members: excludedMe(), state: stateWith({ me_excluded: true }) });
    expect(page.text("#excluded-card")).toContain('This computer was removed from the network "demo-network"');
    expect(page.text("#excluded-card")).toContain("An admin removed it. Sites no longer open through Mesh Town");
    expect(page.attr("#excluded-card", "role")).toBe("status");
    expect(page.has("#peers-list")).toBe(false);
    expect(page.has("[data-member-menu]")).toBe(false);
  });

  it("the Home tab shows the same card", async () => {
    const page = await openMembersPage({ members: excludedMe(), state: stateWith({ me_excluded: true }), tab: "home" });
    expect(page.text("#excluded-card")).toContain('This computer was removed from the network "demo-network"');
  });

  it("the header says it too", async () => {
    const page = await openMembersPage({ members: excludedMe(), state: stateWith({ me_excluded: true }) });
    expect(page.statusLine()).toContain('Removed from the network "demo-network" by an admin — sites won\'t open through Mesh Town');
  });

  it("Home has no card button: the invite field is already open under the card", async () => {
    const page = await openMembersPage({ members: excludedMe(), state: stateWith({ me_excluded: true }), tab: "home" });
    expect(page.has("#excluded-join")).toBe(false);
    expect(page.has("#join-key-input")).toBe(true);
  });

  it("`me_excluded` in the state alone is enough: the node does not know the list (404)", async () => {
    const page = await openMembersPage({ members: null, state: stateWith({ me_excluded: true }) });
    expect(page.has("#excluded-card")).toBe(true);
    expect(page.statusLine()).not.toContain("is not responding");
  });

  it("`me_excluded` in the list alone is enough, too", async () => {
    const page = await openMembersPage({ members: excludedMe(), state: stateWith() });
    expect(page.has("#excluded-card")).toBe(true);
  });

  it("`me_excluded` by refusals alone gets the removed wording", async () => {
    const refusing = peerOneRow({ online: false, last_refusal: { side: "them", reason: "excluded", at_unix: 1 } });
    const members = membersReply({ rows: [selfRow(), refusing], me_excluded: true });
    const page = await openMembersPage({ members, state: stateWith({ me_excluded: true }) });
    expect(page.text("#excluded-card")).toContain('This computer was removed from the network "demo-network"');
    expect(page.text("#excluded-card")).toContain("An admin removed it. Sites no longer open through Mesh Town");
    expect(page.text("#excluded-card")).not.toContain("You left the network");
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
    expect(page.text("#excluded-card")).not.toContain("was banned from the network");
    expect(page.has("[data-member-menu]")).toBe(false);
  });
});

describe("somebody else left the network", () => {
  it("is in the group \"No longer in the network\" with the line \"left the network\"", async () => {
    const gone = peerOneRow({ status: "excluded", online: false, denied_by_me: "excluded", excluded: excludedInfo({ how: "left", voters: [] }) });
    const page = await openMembersPage({ members: membersReply({ rows: [selfRow(), gone, peerTwoNamed()] }) });
    expect(page.listCodes()).toEqual([ME.code, "cccc3333"]);
    expect(page.summaryOf("#excluded-list")).toBe("No longer in the network (1)");
    expect(page.text("#excluded-list li")).toMatch(/^peer-one · left the network on \d{1,2} Oct$/);
  });
});

describe("the leave dialog with several networks", () => {
  describe("on the tab of the second network", () => {
    it("names that network in the title and says the others stay", async () => {
      const dialog = await dialogOnSecond();
      expect(dialog.title()).toBe('Leave "my devices"?');
      expect(dialog.body()).toContain('Disconnect this computer from "my devices" for good. Your other networks stay. You\'ll need a new invite to come back.');
      expect(dialog.yes()).toBe("Leave network");
      expect(dialog.cancel()).toBe("Cancel");
      expect(dialog.focusIsOnCancel()).toBe(true);
    });

    it("does not use the wording of the single network", async () => {
      const dialog = await dialogOnSecond();
      expect(dialog.title()).not.toContain("Leave the network");
      expect(dialog.body()).not.toContain("sites stop opening through Mesh Town");
    });

    it("Cancel sends nothing", async () => {
      const dialog = await dialogOnSecond();
      await dialog.whenCancelled();
      expect(dialog.leaveRequests()).toEqual([]);
    });
  });

  it("on the tab of the first network names that one", async () => {
    const dialog = await dialogOnSecond({ hash: "peers" });
    expect(dialog.title()).toBe('Leave "friends"?');
  });

  describe("your exit is in the network being left", () => {
    it("says so, with the name of the exit, and what happens to the sites", async () => {
      const dialog = await dialogOnSecond({ exit: EXTRA_FULL });
      expect(dialog.body()).toContain("Your exit old-laptop is in this network. After you leave, sites will open directly until you choose another exit.");
    });

    it("does not add the common tail \"is your exit\"", async () => {
      const dialog = await dialogOnSecond({ exit: EXTRA_FULL });
      expect(dialog.body()).not.toContain("is your exit");
    });
  });

  describe("your exit is in another network", () => {
    it("the dialog does not mention the exit", async () => {
      const dialog = await dialogOnSecond({ exit: PEER_FULL });
      expect(dialog.body()).not.toContain("Your exit");
      expect(dialog.body()).not.toContain("is your exit");
    });
  });
});

describe("the leave dialog with one network", () => {
  it("keeps the wording of the single network and does not talk about other networks", async () => {
    const page = await openMembersPage();
    await page.openMenu(SELF_FULL);
    await page.pick("leave");
    expect(page.dialogBody()).not.toContain("Your other networks stay");
    expect(page.dialogBody()).not.toContain("Your exit");
    page.expectDialog({ title: 'Leave the network "demo-network"?', action: "Leave network" });
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

const excludedMe = () => membersReply({ rows: [mineExcluded("removed", [PEER_FULL, PEER2_FULL]), peerOneRow(), peerTwoNamed()], me_excluded: true });
const leftMe = () => membersReply({ rows: [mineExcluded("left", []), peerOneRow(), peerTwoNamed()], me_excluded: true });


// Two networks: A is `main` ("friends"), B is `k7q2xa` ("my devices", its other member is old-laptop). `exit`: the chosen exit.
// Opens the page on B (or on `hash`), picks "Leave network…" from your row and returns what the dialog shows.
async function dialogOnSecond({ hash = "peers/" + NET_B, exit = null } = {}) {
  const node = createFakeNode(twoNetsWithExit(exit));
  const { wrapper } = await mountPage(node, { hash });
  const click = async (w) => { await w.trigger("click"); await settle(); };
  await click(wrapper.get('#peers-list button[data-member-menu="' + SELF_FULL + '"]'));
  await click(wrapper.get('[role="menu"] [data-member-item="leave"]'));
  const text = (selector) => wrapper.get(selector).text();
  return {
    title: () => text("#confirm-title"),
    body: () => text("#confirm-body"),
    yes: () => text("#member-confirm-yes"),
    cancel: () => text("#member-confirm-cancel"),
    focusIsOnCancel: () => document.activeElement === wrapper.get("#member-confirm-cancel").element,
    whenCancelled: () => click(wrapper.get("#member-confirm-cancel")),
    leaveRequests: () => node.calls.filter((c) => c.method === "POST" && c.path.endsWith("/members/leave")),
  };
}

const selfPeer = () => ({ self: true, online: true, id: "aaaa1111", id_full: SELF_FULL, name: "finch-unwary", country: "RU", version: "0.1.0" });
const laptopPeer = () => ({ id: "dddd4444", id_full: EXTRA_FULL, name: "old-laptop", country: "NL", version: "0.1.0", online: true, path: "direct", rtt_ms: 20, exit: false });

function twoNetsWithExit(exit) {
  const base = baseState({ network: { id_hex: "net-bbbb", label: "my devices", masked: "mesh2-••••-••••" }, peers: [selfPeer(), laptopPeer()] });
  const secondNet = { state: { ...base, members: { count: 1, gossip_neighbors: 0, me_excluded: false } }, members: membersReply({ rows: [selfRow(), extraRow()] }) };
  const opts = twoNets(secondNet);
  const route = { exits: exit ? [exit] : [] };
  return { ...opts, state: { ...opts.state, network: { ...opts.state.network, label: "friends" } }, config: baseConfig({ route, ports: { published: [{ port: 8080, for: "all" }], opened: [] } }) };
}
