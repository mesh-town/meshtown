/* Adding a network from the "Members" page: the link under the list of a single network, the "+" tab of several, the form
 * (`#add-net-card`), the banner on the tab of the new network. The whole page runs on a fake node with A = `main` "friends",
 * B = `k7q2xa` "my devices" and one empty slot that `POST /api/nets` hands out; a join gives that slot the label "work". */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { resetNetBanner } from "../home/add-net-banner.ts";
import { shown } from "../testing/dom.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { membersReply, selfRow } from "../testing/members-fixtures.mjs";
import { NET_A, NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, polls, settle, unmountPage } from "../testing/page.mjs";
import { PEER_FULL, baseConfig, baseState } from "../testing/section.mjs";

const FRESH = "neta22";
const INVITE = "mesh-invite2-ABCDEFGH";
const ADD_LINK = "Add another network";
const CREATE_TOGGLE = "No invite? Create a new network ▸";
const JOINED = 'You joined "work".';
const EXIT_TAIL = "Your exit hasn't changed: sites still open via peer-one (friends).";
const CREATED = 'You created "work". Nobody else is here yet: use Invite to "work" to add a member.';

beforeEach(() => {
  localStorage.clear();
  resetNetBanner();
});
afterEach(unmountPage);

describe("one network", () => {
  it("offers \"Add another network\" under the list, without the form", async () => {
    const page = await openOne();
    expect(page.link().attributes("href")).toBe("#peers/add");
    expect(page.card().exists(), "the form comes after the click").toBe(false);
    expect(page.isAfter(page.list().element, page.link().element), "the link follows the list").toBe(true);
  });

  it("the link opens the form under the list, the list stays", async () => {
    const page = await openOne();
    await page.whenLinkClicked();
    expect(location.hash).toBe("#peers/add");
    expect(page.card().exists()).toBe(true);
    expect(page.codes(), "the members are still there").toContain("bbbb2222");
    expect(page.isAfter(page.list().element, page.card().element), "the form follows the list").toBe(true);
  });

  it("opened by the address, shows the form the same way", async () => {
    const page = await openOne("peers/add");
    expect(page.card().exists()).toBe(true);
    expect(page.codes()).toContain("bbbb2222");
    expect(page.tabs(), "no row of tabs yet").toHaveLength(0);
  });
});

describe("several networks", () => {
  it("the \"+\" tab shows only the form: no members, no header of a network", async () => {
    const page = await openTwo();
    await page.whenPlusClicked();
    expect(location.hash).toBe("#peers/add");
    expect(page.card().exists()).toBe(true);
    expect(page.wrapper.find("#peers-list").exists(), "list").toBe(false);
    expect(page.wrapper.find("#peers-tabhead").exists(), "header").toBe(false);
    expect(page.selectedName()).toBe("+");
  });

  it("has no \"Add another network\" link: the \"+\" tab does that", async () => {
    const page = await openTwo();
    expect(page.link().exists()).toBe(false);
  });

  it("\"Cancel\" goes back to the first network", async () => {
    const page = await openTwo("peers/add");
    await page.whenClicked("Cancel");
    await page.hashChanged();
    expect(location.hash).toBe("#peers/" + NET_A);
    expect(page.selectedName()).toContain("friends");
  });
});

describe("joining by invite", () => {
  describe("success", () => {
    it("from one network: the new tab opens with the banner", async () => {
      const page = await openOne("peers/add");
      await page.whenJoined();
      expect(location.hash).toBe("#peers/" + FRESH);
      expect(page.tabs()).toHaveLength(3);
      expect(page.selectedName()).toContain("work");
      expect(page.banner()).toBe(JOINED);
    });

    it("from two networks: the new tab opens with the banner", async () => {
      const page = await openTwo("peers/add");
      await page.whenJoined();
      expect(location.hash).toBe("#peers/" + FRESH);
      expect(page.tabs()).toHaveLength(4);
      expect(page.selectedName()).toContain("work");
      expect(page.banner()).toBe(JOINED);
    });

    it("asks for a slot and joins on its address", async () => {
      const page = await openTwo("peers/add");
      await page.whenJoined();
      expect(page.posted()).toEqual([
        ["/api/nets", {}],
        ["/api/net/" + FRESH + "/network/join", { key: INVITE }],
      ]);
    });

    it("\"OK\" removes the banner", async () => {
      const page = await openTwo("peers/add");
      await page.whenJoined();
      await page.whenBannerClosed();
      expect(page.banner()).toBeNull();
    });

    it("the banner belongs to the new tab: another tab has none", async () => {
      const page = await openTwo("peers/add");
      await page.whenJoined();
      await page.whenTabClicked("friends");
      expect(page.banner()).toBeNull();
    });

    it("when the exit is not direct, adds that it has not changed", async () => {
      const page = await openTwo("peers/add", { exit: true });
      await page.whenJoined();
      expect(page.banner()).toBe(JOINED + " " + EXIT_TAIL);
    });

    it("when sites open directly, says nothing about the exit", async () => {
      const page = await openTwo("peers/add");
      await page.whenJoined();
      expect(page.banner()).not.toContain("exit");
    });
  });

  describe("the node waits for members", () => {
    it("shows the network as one tab \"New network · joining…\" and stays on the form", async () => {
      const page = await openTwo("peers/add", { waiting: true });
      await page.whenJoined();
      expect(page.joiningTabs(), "one tab for the waiting network").toHaveLength(1);
      expect(compact(page.joiningTabs()[0].text())).toMatch(/^New network\s*·\s*joining…$/);
      expect(page.joiningTabs()[0].attributes("href"), "the tab is a link to the slot").toBe("#peers/" + FRESH);
      expect(page.tabsText().match(/joining…/g)).toHaveLength(1);
      expect(location.hash).toBe("#peers/add");
      expect(page.card().exists()).toBe(true);
    });

    it("the form shows the waiting card with Cancel", async () => {
      const page = await openTwo("peers/add", { waiting: true });
      await page.whenJoined();
      expect(page.card().find("#net-joining").exists()).toBe(true);
      expect(page.card().find("#net-join-cancel").exists()).toBe(true);
      expect(page.card().find("#add-net-cancel").exists(), "one Cancel on the screen").toBe(false);
    });

    it("the tab of the waiting network shows the waiting card with Cancel, not the \"no network\" text", async () => {
      const page = await openTwo("peers/add", { waiting: true });
      await page.whenJoined();
      await page.whenTabClicked("New network");
      expect(location.hash).toBe("#peers/" + FRESH);
      expect(page.selectedName()).toContain("New network");
      expect(page.wrapper.find("#sec-peers #net-joining").exists(), "the waiting card").toBe(true);
      expect(page.wrapper.find("#sec-peers #net-join-cancel").exists(), "Cancel").toBe(true);
      expect(page.wrapper.find("[data-no-network-msg]").exists(), "no \"no network\" text").toBe(false);
      expect(page.card().exists(), "not the form").toBe(false);
    });

    it("Cancel on the tab of the waiting network removes the tab and leaves the network", async () => {
      const page = await openTwo("peers/add", { waiting: true });
      await page.whenJoined();
      await page.whenTabClicked("New network");
      await page.whenJoinCancelled();
      expect(page.joiningTabs()).toHaveLength(0);
      expect(page.tabsText()).not.toContain("New network");
      expect(page.wrapper.find("#sec-peers #net-joining").exists()).toBe(false);
      expect(page.posted().at(-1)).toEqual(["/api/net/" + FRESH + "/members/leave", { confirm: true }]);
      expect(page.wrapper.find("#peers-list").exists(), "a network with members is on show").toBe(true);
    });

    it("Cancel in the form on the \"add\" tab leaves the form open and empty, without a tab for the network", async () => {
      const page = await openTwo("peers/add", { waiting: true });
      await page.whenJoined();
      await page.whenJoinCancelled();
      expect(page.card().exists(), "the form stays open").toBe(true);
      expect(page.card().find("#net-joining").exists()).toBe(false);
      expect(page.fieldValue()).toBe("");
      expect(page.joiningTabs()).toHaveLength(0);
      expect(page.tabs(), "friends, my devices and \"+\"").toHaveLength(3);
      expect(page.posted().at(-1)).toEqual(["/api/net/" + FRESH + "/members/leave", { confirm: true }]);
    });

    it("when the network arrives, its tab opens by itself with the banner", async () => {
      const page = await openTwo("peers/add", { waiting: true });
      await page.whenJoined();
      page.arrives();
      await polls(page.node, 1);
      await page.hashChanged();
      expect(location.hash).toBe("#peers/" + FRESH);
      expect(page.selectedName()).toContain("work");
      expect(page.tabsText()).not.toContain("joining…");
      expect(page.banner()).toBe(JOINED);
      expect(page.card().exists(), "the form and its waiting line are gone").toBe(false);
    });

    it("when the user is on another tab, the page stays there and the banner waits on the new tab", async () => {
      const page = await openTwo("peers/add", { waiting: true });
      await page.whenJoined();
      await page.whenTabClicked("friends");
      page.arrives();
      await polls(page.node, 1);
      await page.hashChanged();
      expect(location.hash).toBe("#peers/main");
      expect(page.banner()).toBeNull();
      await page.whenTabClicked("work");
      expect(page.banner()).toBe(JOINED);
    });

    it("when no poll saw the slot as joining, the network still arrives on its tab", async () => {
      const page = await openTwo("peers/add", { waiting: "unseen" });
      await page.whenJoined();
      page.arrives();
      await polls(page.node, 1);
      await page.hashChanged();
      expect(location.hash).toBe("#peers/" + FRESH);
      expect(page.banner()).toBe(JOINED);
    });
  });

  describe("the network is already added", () => {
    it("says so and sends no more requests", async () => {
      const page = await openTwo("peers/add", { duplicate: true });
      await page.whenJoined();
      expect(page.cardText()).toContain("You're already in \"my devices\" — this invite isn't needed. Keep it for someone else.");
      expect(page.posted().map(([path]) => path)).toEqual(["/api/nets", "/api/net/" + FRESH + "/network/join"]);
      expect(page.banner()).toBeNull();
    });

    it("\"Open\" leads to the tab of that network", async () => {
      const page = await openTwo("peers/add", { duplicate: true });
      await page.whenJoined();
      await page.whenClicked('Open "my devices"');
      await page.hashChanged();
      expect(location.hash).toBe("#peers/" + NET_B);
      expect(page.selectedName()).toContain("my devices");
      expect(page.card().exists(), "the form is gone").toBe(false);
    });
  });
});

describe("creating a network", () => {
  it("opens the tab of the new network with its own banner", async () => {
    const page = await openTwo("peers/add");
    await page.whenCreated("work");
    expect(location.hash).toBe("#peers/" + FRESH);
    expect(page.selectedName()).toContain("work");
    expect(page.banner()).toBe(CREATED);
    expect(page.posted()).toEqual([
      ["/api/nets", {}],
      ["/api/net/" + FRESH + "/network/create", { label: "work" }],
    ]);
  });
});

const compact = (text) => text.replace(/\s+/g, " ").trim();

// The fixture's published port has no audience; real entries always have one.
const audienced = (opts) => ({ ...opts, config: baseConfig({ ports: { published: [{ port: 8080, for: "all" }], opened: [] } }) });

// Network A holds an exit that is a member of A.
function withExit(opts) {
  const exit = { id: "bbbb2222", id_full: PEER_FULL, name: "peer-one", country: "DE", exit: true, online: true, active: true };
  opts.state = { ...opts.state, route: { state: "exit", exits: [exit] } };
  opts.config = baseConfig({ ...opts.config, route: { exits: [PEER_FULL] } });
  return opts;
}

function becomesNetwork(slot) {
  slot.joining = false;
  slot.label = "work";
  slot.id_hex = "net-cccc";
  slot.state = baseState({ network: { id_hex: "net-cccc", masked: "mesh1-••••-••••" }, peers: [] });
  slot.routes["GET /api/members"] = membersReply({ rows: [selfRow()] });
}

function scriptSlot(slot, { waiting, duplicate }) {
  if (duplicate) slot.routes["POST /api/network/join"] = { status: 409, body: { error: "net_already_added", slot: NET_B } };
  else if (waiting) slot.routes["POST /api/network/join"] = () => { slot.joining = waiting !== "unseen"; return { label: null, restart: false, waiting: true }; };
  else slot.routes["POST /api/network/join"] = () => { becomesNetwork(slot); return { label: "work", restart: false }; };
  slot.routes["POST /api/network/create"] = () => { becomesNetwork(slot); return { key: "mesh1-secret", label: "work", restart: false }; };
}

async function openOne(hash = "peers") {
  const opts = audienced(twoNets());
  opts.slots = [opts.slots[0], { id: FRESH }];
  return mountOn(createFakeNode(opts), hash, {});
}

async function openTwo(hash = "peers", { exit = false, waiting = false, duplicate = false } = {}) {
  const opts = audienced(twoNets());
  opts.slots.push({ id: FRESH });
  return mountOn(createFakeNode(exit ? withExit(opts) : opts), hash, { waiting, duplicate });
}

async function mountOn(node, hash, script) {
  scriptSlot(node.slotList.find((s) => s.id === FRESH), script);
  const { wrapper, store } = await mountPage(node, { hash });
  return pageOf(node, wrapper, store);
}

function pageOf(node, wrapper, store) {
  const section = () => wrapper.get("#sec-peers");
  const tabs = () => wrapper.findAll("#sec-peers .ntabs a");
  const tabNamed = (name) => tabs().find((t) => compact(t.text()).startsWith(name));
  const card = () => wrapper.find("#add-net-card[data-context=\"peers\"]");
  const buttonIn = (root, name) => root.findAll("button, a").find((b) => compact(b.text()) === name);
  const hashChanged = async () => {
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    await vi.advanceTimersByTimeAsync(0);
    await settle();
  };
  const click = async (w) => {
    await w.trigger("click");
    await vi.advanceTimersByTimeAsync(0);
    await settle();
  };
  const whenClicked = async (name) => {
    const control = buttonIn(card(), name);
    expect(control, "a control named " + name + " in the form").toBeDefined();
    await click(control);
  };
  const clickLink = async (el) => {
    await click(el);
    if (location.hash !== el.attributes("href")) location.hash = el.attributes("href");
    await hashChanged();
  };
  const bannerOk = () => section().findAll("button").find((b) => {
    const around = b.element.parentElement?.textContent ?? "";
    return compact(b.text()) === "OK" && /You (joined|created)/.test(around) && !around.includes("You’re now in");
  });
  const typed = async (value) => {
    const field = card().findAll("input").find((i) => shown(i.element));
    await field.setValue(value);
  };
  return {
    node, wrapper, store, tabs, card, hashChanged, whenClicked,
    link: () => section().findAll("a").find((a) => compact(a.text()) === ADD_LINK) ?? { exists: () => false },
    list: () => wrapper.get("#peers-list"),
    codes: () => wrapper.findAll("#sec-peers #peers-list .peer-code").map((c) => c.text()),
    cardText: () => compact(card().text()),
    joiningTabs: () => tabs().filter((t) => t.text().includes("joining…")),
    fieldValue: () => card().findAll("input").find((i) => shown(i.element)).element.value,
    tabsText: () => compact(wrapper.get("#sec-peers .ntabs").text()),
    selectedName: () => compact(tabs().find((t) => t.attributes("aria-current") === "page")?.text() ?? ""),
    isAfter: (first, second) => Boolean(first.compareDocumentPosition(second) & Node.DOCUMENT_POSITION_FOLLOWING),
    banner() {
      const ok = bannerOk();
      return ok ? compact(ok.element.parentElement.textContent).replace(/ ?OK$/, "") : null;
    },
    posted: () => node.calls.filter((c) => c.method === "POST" && c.path !== "/api/network/inspect").map((c) => [c.path, c.body]),
    arrives: () => becomesNetwork(node.slotList.find((s) => s.id === FRESH)),
    whenLinkClicked: () => clickLink(section().findAll("a").find((a) => compact(a.text()) === ADD_LINK)),
    whenPlusClicked: () => clickLink(tabs().at(-1)),
    whenTabClicked: (name) => clickLink(tabNamed(name)),
    whenBannerClosed: () => click(bannerOk()),
    whenJoinCancelled: () => click(wrapper.get("#net-join-cancel")),
    async whenJoined() {
      await typed(INVITE);
      await whenClicked("Add network");
      await hashChanged();
      await polls(node, 1);
    },
    async whenCreated(name) {
      await whenClicked(CREATE_TOGGLE);
      await typed(name);
      await whenClicked("Create network");
      await hashChanged();
      await polls(node, 1);
    },
  };
}
