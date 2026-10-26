/* The "Members" tab on a node with several networks: the whole page on a fake node with two slots.
 * Network A is `main` ("friends", peer-one online, peer-two offline), network B is `k7q2xa` ("my devices", two other members online).
 * Contract: the tab row is `.ntabs a` (the last one is the "+"), the header of the current tab is `#peers-tabhead`
 * (its button asks for an invite), the list and the status line are the ones of the single-network page.
 * One network: no tab row, "Invite a member" above the list, nothing is sent to `/api/net/…`. Two or more: the data and the
 * actions of the current tab go to its own slot address. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { visibleText } from "../testing/dom.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { EXTRA_FULL, actionReply, excludedInfo, memberRow, membersReply, pendingRow, peerOneRow, peerTwoRow, selfRow } from "../testing/members-fixtures.mjs";
import { NET_A, NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, polls, settle, unmountPage } from "../testing/page.mjs";
import { PEER2_FULL, SELF_FULL, baseConfig, baseState } from "../testing/section.mjs";

const HINT_KEY = "meshtown.netsHintSeen";
const HINT = "You’re now in 2 networks.";
const COPIED = "Invite copied. It works once, for 24 hours — send it to one person in a private message.";
const SECRET = "mesh-invite2-SECRETSECRET";
const FULL_E = "E".repeat(64);

beforeEach(() => localStorage.clear());
afterEach(() => {
  unmountPage();
  Reflect.deleteProperty(navigator, "clipboard");
});

describe("one network", () => {
  it("has a row of its own tab, active, and \"+\"", async () => {
    const page = await openOne();
    expect(page.tabs()).toHaveLength(2);
    expect(page.tabs()[0].classes()).toContain("active");
    expect(page.tabs()[0].attributes("aria-current")).toBe("page");
    expect(page.tabs()[1].text()).toBe("+");
    expect(page.wrapper.find("#peers-nets-hint").exists(), "no hint with one network").toBe(false);
  });

  it("offers \"Invite a member\" above the list", async () => {
    const page = await openOne();
    expect(page.tabHead().get("button").text()).toBe("Invite a member");
    const position = page.tabHead().element.compareDocumentPosition(page.list().element);
    expect(position & Node.DOCUMENT_POSITION_FOLLOWING, "the header comes before the list").toBeTruthy();
  });

  it("shows no hint about several networks", async () => {
    const page = await openOne();
    expect(page.sectionText()).not.toContain(HINT);
  });

  it("never asks for a network address, only the plain one", async () => {
    const page = await openOne();
    await page.whenInviteClicked();
    expect(page.addressedPaths().filter((p) => p.startsWith("/api/net/"))).toEqual([]);
    expect(page.invitePaths()).toEqual(["/api/network/invite"]);
  });
});

describe("two networks", () => {
  describe("the row of tabs", () => {
    it("has a tab per network and the \"+\"", async () => {
      const page = await openTwo();
      expect(page.tabs()).toHaveLength(3);
      expect(page.tabNamed("friends").exists()).toBe(true);
      expect(page.tabNamed("my devices").exists()).toBe(true);
      expect(page.tabs()[2].attributes("href")).toBe("#peers/add");
    });

    it("selects the first network when the address has no tab", async () => {
      const page = await openTwo();
      expect(page.selectedName()).toContain("friends");
    });

    it("says how many members are online in each network", async () => {
      const page = await openTwo();
      expect(page.tabNamed("friends").text()).toContain("· 1 online");
      expect(page.tabNamed("my devices").text()).toContain("· 2 online");
      expect(page.tabNamed("my devices").attributes("title")).toBe("my devices — 2 of 2 members online");
    });

    it("says \"no connection\" instead of the count when nobody else is online", async () => {
      const page = await openTwo({ patchB: bNet({ others: [laptop(false), phone(false)] }) });
      expect(page.tabNamed("my devices").text()).toContain("· no connection");
      expect(page.tabNamed("my devices").text()).not.toContain("online");
      expect(page.tabNamed("my devices").attributes("title")).toBe("my devices — no connection");
    });

    it("says \"no connection\" when the network is broken, whoever is online", async () => {
      const page = await openTwo({ patchB: bNet({ status: "broken" }) });
      expect(page.tabNamed("my devices").text()).toContain("· no connection");
    });

    it("counts the join requests of each network in a circle, not the number the summary gives", async () => {
      const page = await openTwo({ patchA: { members: oneRequest() }, patchB: bNet({ admitting: 7, rows: twoRequests() }) });
      expect(page.circleOf("friends").text()).toBe("1");
      expect(page.circleOf("friends").attributes("title")).toBe("1 to confirm");
      expect(page.circleOf("my devices").text()).toBe("2");
      expect(page.circleOf("my devices").attributes("title")).toBe("2 to confirm");
    });

    it("has no circle where nothing waits for you", async () => {
      const page = await openTwo();
      expect(page.circleOf("friends").exists()).toBe(false);
      expect(page.circleOf("my devices").exists()).toBe(false);
    });
  });

  describe("the header of the tab", () => {
    it("on the first network invites to it and tells who is there", async () => {
      const page = await openTwo();
      expect(page.tabHead().get("button").text()).toBe('Invite to "friends"');
      expect(page.tabHead().text()).toContain('Network "friends" · you and 2 others, 1 online');
    });

    it("on the second network invites to that one", async () => {
      const page = await openTwo({ hash: "peers/" + NET_B });
      expect(page.tabHead().get("button").text()).toBe('Invite to "my devices"');
      expect(page.tabHead().text()).toContain('Network "my devices" · you and 2 others, both online');
    });

    it.each([
      ["nobody else", [], "just you"],
      ["one other, online", [laptop(true)], "you and 1 other, online"],
      ["one other, offline", [laptop(false)], "you and 1 other, 0 online"],
      ["two others, both online", [laptop(true), phone(true)], "you and 2 others, both online"],
      ["three others, all online", [laptop(true), phone(true), tablet(true)], "you and 3 others, all online"],
      ["three others, two online", [laptop(true), phone(false), tablet(true)], "you and 3 others, 2 online"],
    ])("describes %s", async (_what, others, who) => {
      const page = await openTwo({ hash: "peers/" + NET_B, patchB: bNet({ others }) });
      expect(page.tabHead().text()).toContain('Network "my devices" · ' + who);
    });
  });

  describe("the hint", () => {
    it("is shown with the number of networks", async () => {
      const page = await openTwo();
      expect(page.sectionText()).toContain(HINT);
      expect(page.sectionText()).toContain("Each network has its own tab here.");
    });

    it("\"OK\" hides it and remembers that it was seen", async () => {
      const page = await openTwo();
      await page.whenHintAcknowledged();
      expect(page.sectionText()).not.toContain(HINT);
      expect(localStorage.getItem(HINT_KEY)).toBe("1");
    });

    it("does not come back after the page is opened again", async () => {
      const first = await openTwo();
      await first.whenHintAcknowledged();
      unmountPage();
      const again = await openTwo();
      expect(again.sectionText()).not.toContain(HINT);
    });

    it("is not shown when it was seen before", async () => {
      localStorage.setItem(HINT_KEY, "1");
      const page = await openTwo();
      expect(page.sectionText()).not.toContain(HINT);
    });

    it("forgets it was seen when the node falls to one network", async () => {
      localStorage.setItem(HINT_KEY, "1");
      const page = await openTwo();
      page.node.slotList = page.node.slotList.filter((s) => s.id !== NET_B);
      await polls(page.node, 2);
      expect(page.tabs(), "only the network and \"+\" are left").toHaveLength(2);
      expect(localStorage.getItem(HINT_KEY)).toBeNull();
    });
  });
});

describe("clicking the second tab", () => {
  it("opens #peers/<slot> and selects the tab", async () => {
    const page = await openTwo();
    await page.whenTabClicked("my devices");
    expect(location.hash).toBe("#peers/" + NET_B);
    expect(page.selectedName()).toContain("my devices");
  });

  it("lists the members of that network, not of the first", async () => {
    const page = await openTwo();
    await page.whenTabClicked("my devices");
    expect(page.codes()).toContain("dddd4444");
    expect(page.codes()).not.toContain("bbbb2222");
  });

  it("sends a member action to that network's address", async () => {
    const page = await openTwo({ patchB: bNet() });
    page.node.slotList[1].routes["POST /api/members/exclude"] = () => actionReply(bMembers([laptop(true), phone(true)]));
    await page.whenTabClicked("my devices");
    await page.whenMemberBanned(EXTRA_FULL);
    expect(page.posted("/api/net/" + NET_B + "/members/exclude")).toEqual([{ id: EXTRA_FULL }]);
    expect(page.posted("/api/members/exclude"), "nothing for the first network").toEqual([]);
  });

  it("returns to the first network when the address goes back", async () => {
    const page = await openTwo();
    await page.whenTabClicked("my devices");
    await page.whenAddressBack();
    expect(page.selectedName()).toContain("friends");
    expect(page.codes()).toContain("bbbb2222");
  });
});

describe("the invite on the second tab", () => {
  it("is asked once, from that network", async () => {
    const page = await openTwo({ hash: "peers/" + NET_B });
    await page.whenInviteClicked();
    expect(page.invitePaths()).toEqual(["/api/net/" + NET_B + "/network/invite"]);
  });

  it("when copied, says so for ten seconds", async () => {
    const page = await openTwo({ hash: "peers/" + NET_B });
    page.clipboardWorks();
    await page.whenInviteClicked();
    expect(page.sectionText()).toContain(COPIED);
    await page.after(9000);
    expect(page.sectionText(), "still shown before the ten seconds pass").toContain(COPIED);
    await page.after(2000);
    expect(page.sectionText()).not.toContain(COPIED);
  });

  it("when the clipboard refuses, shows the invite to copy by hand", async () => {
    const page = await openTwo({ hash: "peers/" + NET_B });
    page.clipboardRefuses();
    await page.whenInviteClicked();
    expect(page.sectionText()).toContain("Clipboard unavailable");
    expect(page.sectionText()).toContain(SECRET);
  });

  it("when the node cannot make it, says that the invite is not ready", async () => {
    const page = await openTwo({ hash: "peers/" + NET_B });
    page.node.slotList[1].routes["GET /api/network/invite"] = { status: 503, body: { error: "unavailable" } };
    await page.whenInviteClicked();
    expect(page.sectionText()).toContain("The invite isn't ready yet");
    expect(page.sectionText()).not.toContain(COPIED);
  });
});

describe("the status under the list", () => {
  it("belongs to the tab where the action was done: the other tab starts empty", async () => {
    const page = await openTwo({ patchA: { members: oneRequest() } });
    page.node.routes["POST /api/members/exclude"] = () => actionReply(afterRemoval());
    await page.whenMemberBanned(PEER_ONE_FULL);
    expect(page.status(), "the action on the first network left a line").not.toBe("");
    await page.whenTabClicked("my devices");
    expect(page.status()).toBe("");
  });

  it("after leaving the second network keeps the phrase on the first one", async () => {
    const page = await openTwo({ hash: "peers/" + NET_B });
    await page.whenLeft();
    expect(page.tabs(), "only the network and \"+\" are left").toHaveLength(2);
    expect(page.codes()).toContain("bbbb2222");
    expect(page.status()).toContain("You left the network");
    await polls(page.node, 1);
    expect(page.status()).toContain("You left the network");
  });
});

describe("the first slot holds no network", () => {
  it("shows the other network as the only tab and its members", async () => {
    const page = await openHiddenMain();
    expect(page.tabs()).toHaveLength(2);
    expect(page.tabs()[0].classes()).toContain("active");
    expect(page.codes()).toContain("dddd4444");
    expect(page.tabHead().get("button").text()).toBe("Invite a member");
  });

  it("asks the invite from the other network", async () => {
    const page = await openHiddenMain();
    await page.whenInviteClicked();
    expect(page.invitePaths()).toEqual(["/api/net/" + NET_B + "/network/invite"]);
  });

  it("shows the other network at #peers/main too", async () => {
    const page = await openHiddenMain("peers/main");
    expect(page.tabs()).toHaveLength(2);
    expect(page.codes()).toContain("dddd4444");
  });
});

describe("networks with the same name", () => {
  it("tell the tabs apart by the start of the network key", async () => {
    const page = await openSameNames();
    expect(compact(page.tabs()[0].text())).toMatch(/friends · 3f2a/);
    expect(compact(page.tabs()[1].text())).toMatch(/friends · 9c01/);
  });

  it("use the same names in the header of the tab", async () => {
    const page = await openSameNames("peers/" + NET_B);
    expect(page.tabHead().get("button").text()).toBe('Invite to "friends · 9c01"');
  });
});

const PEER_ONE_FULL = "B".repeat(64);
const compact = (text) => text.replace(/\s+/g, " ").trim();

const selfPeer = () => ({ self: true, online: true, id: "aaaa1111", id_full: SELF_FULL, name: "finch-unwary", country: "RU", version: "0.1.0" });
const otherPeer = (idFull, id, name, online) => ({ id, id_full: idFull, name, country: "NL", version: "0.1.0", online, path: "direct", rtt_ms: 20, exit: false });
const laptop = (online) => otherPeer(EXTRA_FULL, "dddd4444", "old-laptop", online);
const phone = (online) => otherPeer(PEER2_FULL, "cccc3333", "pocket", online);
const tablet = (online) => otherPeer(FULL_E, "eeee5555", "tablet", online);
const rowOf = (p) => memberRow({ id: p.id, id_full: p.id_full, name: p.name, online: p.online });

const bMembers = (others) => membersReply({ rows: [selfRow(), ...others.map(rowOf)], me_admin: true });
const oneRequest = () => membersReply({ rows: [selfRow(), peerOneRow(), pendingRow()], me_admin: true });
const twoRequests = () => [selfRow(), pendingRow(), pendingRow({ id: "ffff6666", id_full: "F".repeat(64) })];
const afterRemoval = () => membersReply({ rows: [selfRow(), peerOneRow({ status: "excluded", excluded: excludedInfo() }), peerTwoRow()], me_admin: true });

// The spec of B: `others` are its other members, `rows` replace the member rows, `admitting` is the (unused) count of the summary.
function bNet({ others = [laptop(true), phone(true)], rows = null, status = null, admitting = 0, idHex = "net-bbbb", label = "my devices" } = {}) {
  const base = baseState({ network: { id_hex: idHex, masked: "mesh2-••••-••••" }, peers: [selfPeer(), ...others] });
  const node = status ? { ...base.node, status } : base.node;
  const state = { ...base, node, members: { count: others.length, gossip_neighbors: 0, admitting, me_excluded: false } };
  const config = baseConfig({ ports: { published: [{ port: 8080, for: "all" }], opened: [] } });
  return { label, id_hex: idHex, state, config, members: rows ? membersReply({ rows, me_admin: true }) : bMembers(others) };
}

// The fixture's published port has no audience; real entries always have one.
const audienced = (opts) => ({ ...opts, config: baseConfig({ ports: { published: [{ port: 8080, for: "all" }], opened: [] } }) });

function singleNetwork() {
  const opts = audienced(twoNets());
  return { ...opts, slots: [opts.slots[0]] };
}

async function openOne() {
  return mountOn(createFakeNode(singleNetwork()), "peers");
}

async function openTwo({ patchA = null, patchB = bNet(), hash = "peers" } = {}) {
  const opts = audienced(twoNets(patchB));
  if (patchA) opts.slots[0] = { ...opts.slots[0], ...patchA };
  return mountOn(createFakeNode(opts), hash);
}

async function openHiddenMain(hash = "peers") {
  const opts = audienced(twoNets(bNet()));
  opts.slots[0] = { id: NET_A, label: null, id_hex: null, members: null };
  opts.state = baseState({ network: null, peers: [] });
  return mountOn(createFakeNode(opts), hash);
}

async function openSameNames(hash = "peers") {
  const opts = audienced(twoNets(bNet({ label: "friends", idHex: "9c01bbbb" })));
  opts.state = { ...opts.state, network: { ...opts.state.network, id_hex: "3f2a1111" } };
  opts.slots[0] = { ...opts.slots[0], label: "friends", id_hex: "3f2a1111" };
  return mountOn(createFakeNode(opts), hash);
}

async function mountOn(node, hash) {
  node.routes["GET /api/network/invite"] = { invite: SECRET };
  node.slotList.filter((s) => s !== node).forEach((s) => (s.routes["GET /api/network/invite"] = { invite: SECRET }));
  const { wrapper, store } = await mountPage(node, { hash });
  return pageOf(node, wrapper, store);
}

function pageOf(node, wrapper, store) {
  const section = () => wrapper.get("#sec-peers");
  const tabs = () => wrapper.findAll("#sec-peers .ntabs a");
  const tabNamed = (name) => tabs().find((t) => compact(t.text()).startsWith(name)) ?? { exists: () => false };
  const click = async (w) => { await w.trigger("click"); await settle(); };
  const hashChanged = async () => {
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    await vi.advanceTimersByTimeAsync(0);
    await settle();
  };
  const calls = (method, test) => node.calls.filter((c) => c.method === method && test(c.path));
  return {
    node, wrapper, store, tabs, tabNamed,
    tabHead: () => wrapper.get("#peers-tabhead"),
    list: () => wrapper.get("#peers-list"),
    sectionText: () => compact(visibleText(section().element)),
    selectedName: () => compact(tabs().find((t) => t.attributes("aria-current") === "page")?.text() ?? ""),
    circleOf: (name) => (tabNamed(name).exists() ? tabNamed(name).find('[title$="to confirm"]') : { exists: () => false }),
    codes: () => wrapper.findAll("#sec-peers #peers-list .peer-code").map((c) => c.text()),
    status: () => wrapper.get("#peers-exit-status").text(),
    invitePaths: () => calls("GET", (p) => p.endsWith("/network/invite")).map((c) => c.path),
    addressedPaths: () => node.calls.map((c) => c.path),
    posted: (path) => calls("POST", (p) => p === path).map((c) => c.body),
    clipboardWorks: () => Object.defineProperty(navigator, "clipboard", { value: { writeText: async () => {} }, configurable: true }),
    clipboardRefuses: () => Object.defineProperty(navigator, "clipboard", { value: { writeText: async () => { throw new Error("denied"); } }, configurable: true }),
    after: async (ms) => { await vi.advanceTimersByTimeAsync(ms); await settle(); },
    whenInviteClicked: () => click(wrapper.get("#peers-tabhead button")),
    whenHintAcknowledged: () => click(section().findAll("button").find((b) => b.text().trim() === "OK")),
    async whenTabClicked(name) {
      const tab = tabNamed(name);
      await tab.trigger("click");
      if (location.hash !== tab.attributes("href")) location.hash = tab.attributes("href");
      await hashChanged();
    },
    async whenAddressBack() { location.hash = "peers"; await hashChanged(); },
    async whenMemberBanned(idFull) {
      await click(wrapper.get('button[data-member-menu="' + idFull + '"]'));
      await click(wrapper.get('[role="menu"] [data-member-item="exclude"]'));
      await click(wrapper.get("#member-confirm-yes"));
    },
    async whenLeft() {
      await click(wrapper.get('button[data-member-menu="' + SELF_FULL + '"]'));
      await click(wrapper.get('[role="menu"] [data-member-item="leave"]'));
      await click(wrapper.get("#member-confirm-yes"));
    },
  };
}
