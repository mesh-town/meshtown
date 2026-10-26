/* "Home" on a node with two networks (A = `main` "friends", B "my devices"): one row per network with the member count and
 * "Open Members", the exit line and the red exit warnings that name the network; no ring, no invite, no "This computer".
 * With one network the page keeps its former look and requests. The whole page runs on a fake node. */

import { afterEach, describe, expect, it } from "vitest";
import { demoConfig, demoState } from "../testing/demo-node.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, polls, settle, unmountPage } from "../testing/page.mjs";
import { shown } from "../testing/dom.mjs";
import { baseConfig, baseState } from "../testing/section.mjs";

const OPEN_MEMBERS = "Open Members";
const NET_A_NAME = "friends";
const NET_B_NAME = "my devices";
const EXIT_ID = "dddd4444";
const EXIT_FULL = "D".repeat(64);
const INVITE = "mesh-invite2-SECRETSECRET";
const goneText = (net) => `p is no longer in "${net}" — sites won’t open through it.`;
const offlineText = (net) => `p (${net}) is offline — sites won't open until it's back.`;

afterEach(unmountPage);

describe("Home with two networks", () => {
  describe("the page is open", () => {
    it("has one row per network, each with an \"Open Members\" button", async () => {
      const { wrapper } = await setup();
      expect(openButtons(wrapper)).toHaveLength(2);
      expect(rowsOf(wrapper).map((r) => r.text())).toEqual([expect.stringContaining(NET_A_NAME), expect.stringContaining(NET_B_NAME)]);
    });

    it("tells in each row how many members are online", async () => {
      const { wrapper } = await setup();
      for (const row of rowsOf(wrapper)) expect(flat(row), "1 of 2 members online").toContain("1 of 2 online");
    });

    it("\"Open Members\" of the second row leads to the tab of that network", async () => {
      const { wrapper } = await setup();
      expect(await targetOf(openButtons(wrapper)[1])).toBe("#peers/" + NET_B);
    });

    it("\"Open Members\" of the first row leads to the tab of the main network", async () => {
      const { wrapper } = await setup();
      expect(await targetOf(openButtons(wrapper)[0])).toBe("#peers/main");
    });

    it("shows no ring, no invite card and no \"This computer\" card", async () => {
      const { wrapper } = await setup();
      expect(wrapper.find(".ring-mark").exists(), "ring").toBe(false);
      expect(wrapper.find("#home-invite").exists(), "invite").toBe(false);
      expect(wrapper.find("#home-computer").exists(), "this computer").toBe(false);
      expect(wrapper.find("#field-node-name").exists(), "name field").toBe(false);
    });

    it("keeps the exit card and says one exit serves all the networks", async () => {
      const { wrapper } = await setup();
      expect(wrapper.get("#home-exit-title").text()).toContain("Exit via…");
      expect(flat(wrapper.get("#home-exit"))).toContain("(one exit for all your networks)");
    });

    it("hides the paragraph \"Sites go through the one member you choose…\" and keeps the explanation of an exit", async () => {
      const { wrapper } = await setup();
      const hint = wrapper.find("#exit-list-hint");
      expect(!hint.exists() || !shown(hint.element), "the paragraph about the one member is hidden").toBe(true);
      expect(wrapper.get("#exit-explain").text()).toContain("An exit is a member whose internet you use");
    });

    it("heads the group of exit candidates with the network name", async () => {
      const { wrapper } = await setup();
      expect(wrapper.get("#exit-list").text()).toContain(NET_A_NAME);
    });

    it("polls the overview and never the single-network state path", async () => {
      const { node } = await setup();
      expect(node.calls.filter((c) => c.path === "/api/state")).toEqual([]);
      expect(node.count("GET", "/api/overview")).toBeGreaterThan(0);
    });
  });

  describe("a network has no connection", () => {
    it("its row says \"no connection\" instead of the count", async () => {
      const { wrapper } = await setup({ patchB: { state: stateB({ others: "offline" }) } });
      const row = rowsOf(wrapper)[1];
      expect(flat(row)).toContain("no connection");
      expect(flat(row)).not.toContain("online");
    });

    it("its dot is grey, and the dot of the other row is green", async () => {
      const { wrapper } = await setup({ patchB: { state: stateB({ others: "offline" }) } });
      const [rowA, rowB] = rowsOf(wrapper);
      expect(dotOf(rowB), "row of B").toContain("grey");
      expect(dotOf(rowA), "row of A").toContain("green");
    });
  });

  describe("a network has nobody but this computer", () => {
    it("its row says \"just you\"", async () => {
      const { wrapper } = await setup({ patchB: { state: stateB({ others: "none" }) } });
      expect(flat(rowsOf(wrapper)[1])).toContain("just you");
    });
  });

  describe("requests wait in a network", () => {
    it("the row of that network adds \"2 requests to confirm\"", async () => {
      const { wrapper } = await setup({ patchB: { state: stateB({ admitting: 2 }) } });
      expect(flat(rowsOf(wrapper)[1])).toContain("1 of 2 online · 2 requests to confirm");
      expect(flat(rowsOf(wrapper)[0])).not.toContain("to confirm");
    });

    it("a chip \"2 requests to confirm in my devices\" leads to the tab of that network", async () => {
      const { wrapper } = await setup({ patchB: { state: stateB({ admitting: 2 }) } });
      const chip = wrapper.findAll("a, button").find((el) => flat(el).includes("2 requests to confirm in " + NET_B_NAME));
      expect(chip, "chip with the network name").toBeTruthy();
      expect(await targetOf(chip)).toBe("#peers/" + NET_B);
    });

    it("one request reads \"1 request to confirm\", in the singular", async () => {
      const { wrapper } = await setup({ patchB: { state: stateB({ admitting: 1 }) } });
      expect(flat(rowsOf(wrapper)[1])).toContain("1 request to confirm");
      expect(flat(rowsOf(wrapper)[1])).not.toContain("1 requests");
    });

    it("without requests there is no chip", async () => {
      const { wrapper } = await setup();
      expect(wrapper.text()).not.toContain("to confirm in");
    });
  });

  describe("both networks have the same name", () => {
    it("each name gets the first four characters of its network key", async () => {
      const { wrapper } = await setup({ clash: true });
      const names = rowsOf(wrapper).map((r) => flat(r));
      expect(names[0]).toContain("friends · 3f2a");
      expect(names[1]).toContain("friends · 9c01");
    });
  });

  describe("sites go through a member of the first network", () => {
    it("the line names the member and the network: \"Sites open via p (friends)\"", async () => {
      const { wrapper } = await setup({ exit: "active" });
      expect(flat(wrapper.get("#sec-home"))).toContain("Sites open via p (friends)");
    });
  });

  describe("sites go through a member of the network that comes first on the page", () => {
    it("the line names that network: \"Sites open via p (my devices)\"", async () => {
      const { wrapper } = await setup({ exit: "active", firstIsB: true });
      expect(flat(wrapper.get("#sec-home"))).toContain("Sites open via p (my devices)");
    });
  });

  describe("no exit is chosen", () => {
    it("the line says \"Sites open directly\"", async () => {
      const { wrapper } = await setup();
      expect(flat(wrapper.get("#sec-home"))).toContain("Sites open directly");
    });
  });

  describe("the exit is no longer in its network (removed)", () => {
    it("a red warning says it is no longer in the network and sites won't open through it", async () => {
      const { wrapper } = await setup({ exit: "removed" });
      const text = flat(wrapper.get("#sec-home"));
      expect(text).toContain("is no longer in");
      expect(text).toContain(goneText(NET_A_NAME));
      expect(text, "removed is not offline").not.toContain("is offline");
    });

    it("names the network of the second slot when that slot holds the exit", async () => {
      const { wrapper } = await setup({ exit: "removed", firstIsB: true });
      expect(flat(wrapper.get("#sec-home"))).toContain(goneText(NET_B_NAME));
    });

    it("offers \"Choose another\"", async () => {
      const { wrapper } = await setup({ exit: "removed" });
      expect(startingWith(wrapper, "Choose another").length, "button \"Choose another…\"").toBeGreaterThan(0);
    });
  });

  describe("the exit member is offline (not removed)", () => {
    it("a red warning names the member and the network and says it is offline", async () => {
      const { wrapper } = await setup({ exit: "offline" });
      const text = flat(wrapper.get("#sec-home"));
      expect(text).toContain(offlineText(NET_A_NAME));
      expect(text, "offline is not removed").not.toContain("is no longer in");
    });

    it("names the network of the second slot when that slot holds the exit", async () => {
      const { wrapper } = await setup({ exit: "offline", firstIsB: true });
      expect(flat(wrapper.get("#sec-home"))).toContain(offlineText(NET_B_NAME));
    });

    it("offers \"Choose another\"", async () => {
      const { wrapper } = await setup({ exit: "offline" });
      expect(startingWith(wrapper, "Choose another").length, "button \"Choose another…\"").toBeGreaterThan(0);
    });
  });
});

describe("Home with one network", () => {
  describe("the node has one network", () => {
    it("shows the ring, the invite card and \"This computer\", and no network rows", async () => {
      const { wrapper } = await setupSingle();
      expect(wrapper.find(".ring-mark").exists(), "ring").toBe(true);
      expect(wrapper.find("#home-invite").exists(), "invite").toBe(true);
      expect(wrapper.find("#home-computer").exists(), "this computer").toBe(true);
      expect(openButtons(wrapper)).toHaveLength(0);
    });

    it("has the former exit card: no network in the title, both paragraphs are there", async () => {
      const { wrapper } = await setupSingle();
      expect(flat(wrapper.get("#home-exit"))).not.toContain("one exit for all your networks");
      expect(wrapper.get("#exit-explain").text()).toContain("An exit is a member whose internet you use");
      expect(shown(wrapper.get("#exit-list-hint").element), "the paragraph about the one member").toBe(true);
      expect(wrapper.get("#exit-list-hint").text()).toContain("Sites go through the one member you choose");
    });

    it("does not head the exit candidates with a network name", async () => {
      const { wrapper } = await setupSingle();
      expect(wrapper.get("#exit-list").text()).not.toContain("demo-network");
    });

    it("asks the node for the overview and never for a network path", async () => {
      const { node } = await setupSingle();
      expect(node.count("GET", "/api/overview")).toBeGreaterThan(0);
      expect(node.calls.filter((c) => c.path.startsWith("/api/net/")), "no /api/net/ requests").toEqual([]);
    });
  });

  describe("the only visible network is the second slot (the main one holds no key)", () => {
    it("the invite is requested in that slot", async () => {
      const { wrapper, node } = await setupSecondOnly();
      await wrapper.get("#netkey-invite-btn").trigger("click");
      await settle();
      const asked = node.calls.filter((c) => c.method === "GET" && c.path.endsWith("/network/invite"));
      expect(asked.map((c) => c.path)).toEqual(["/api/net/" + NET_B + "/network/invite"]);
      expect(wrapper.get("#invite-value").text()).toBe(INVITE);
    });

    it("the computer name is written into that slot's config", async () => {
      const { wrapper, node } = await setupSecondOnly();
      const field = wrapper.get("#field-node-name");
      await field.setValue("laptop-two");
      await field.trigger("blur");
      await settle();
      const puts = node.calls.filter((c) => c.method === "PUT" && c.path.endsWith("/config"));
      expect(puts.map((c) => c.path), "only the slot's config is written").toEqual(["/api/net/" + NET_B + "/config"]);
      expect(puts[0].body.config.node.name).toBe("laptop-two");
    });

    it("the page shows no network rows", async () => {
      const { wrapper } = await setupSecondOnly();
      expect(openButtons(wrapper)).toHaveLength(0);
    });
  });
});

// The second network's `state`: others are online (default), all offline, or absent; `admitting` — requests to confirm.
function stateB({ others = "online", admitting = 0 } = {}) {
  const base = baseState({ network: { id_hex: "net-bbbb", masked: "mesh2-••••-••••" } });
  const me = base.peers.find((p) => p.self);
  const rest = others === "none" ? [] : base.peers.filter((p) => !p.self).map((p) => ({ ...p, online: others === "online" && p.id === "bbbb2222" }));
  return { ...base, peers: [me, ...rest], members: { admitting, me_excluded: false, me_pending: false } };
}

const exitPeer = (online) => ({ id: EXIT_ID, id_full: EXIT_FULL, name: "p", country: "NL", version: null, online, path: "direct", rtt_ms: 20, exit: true });

// The chosen exit of the main network: a member that lives in B.
const chosen = (over) => ({ id: EXIT_ID, id_full: EXIT_FULL, name: "p", country: "NL", exit: true, online: true, active: true, ...over });

const EXITS = {
  active: { route: { state: "exit", exits: [chosen({})] }, online: true },
  offline: { route: { state: "unavailable", exits: [chosen({ online: false, active: false })] }, online: false },
  removed: { route: { state: "unavailable", exits: [chosen({ online: false, active: false, removed: true })] }, online: false },
};

// `patchB` goes into the second network; `clash` gives both networks the label "friends"; `exit` chooses an exit that the
// network on top of the page holds (A, or B when `firstIsB` puts it first).
async function setup({ patchB = {}, clash = false, exit = null, firstIsB = false } = {}) {
  localStorage.clear();
  const opts = twoNets(patchB);
  if (clash) nameBoth(opts);
  if (exit) chooseExit(opts, EXITS[exit], firstIsB);
  if (firstIsB) opts.slots.reverse();
  const node = createFakeNode(opts);
  const { wrapper } = await mountPage(node);
  await polls(node, 1);
  return { wrapper, node };
}

function nameBoth(opts) {
  const [a, b] = opts.slots;
  Object.assign(a, { label: "friends", id_hex: "3f2a0000" });
  Object.assign(b, { label: "friends", id_hex: "9c010000" });
  opts.state.network = { ...opts.state.network, id_hex: a.id_hex };
  b.state.network = { ...b.state.network, id_hex: b.id_hex };
}

function chooseExit(opts, { route, online }, inB) {
  const holder = inB ? opts.slots[1] : opts;
  holder.state = { ...holder.state, peers: [...holder.state.peers, exitPeer(online)], route };
}

async function setupSingle() {
  localStorage.clear();
  const node = createFakeNode({ state: demoState(), config: demoConfig() });
  const { wrapper } = await mountPage(node);
  await polls(node, 1);
  return { wrapper, node };
}

// The main slot holds no network; the second one does.
async function setupSecondOnly() {
  localStorage.clear();
  const opts = twoNets();
  const second = opts.slots[1];
  second.state = { ...second.state, ports: { published: [], network: [] } };
  second.config = baseConfig();
  opts.state = baseState({ network: null, peers: [], domains: [] });
  opts.slots[0] = { id: "main", label: null, id_hex: null, members: null };
  const node = createFakeNode(opts);
  node.slotList.find((s) => s.id === NET_B).routes["GET /api/network/invite"] = { invite: INVITE };
  const { wrapper } = await mountPage(node);
  await polls(node, 1);
  return { wrapper, node };
}

const flat = (w) => w.text().replace(/\s+/g, " ").trim();
const clickables = (wrapper) => wrapper.findAll("a, button");
const buttonsWith = (wrapper, text) => clickables(wrapper).filter((el) => flat(el) === text);
const startingWith = (wrapper, text) => clickables(wrapper).filter((el) => flat(el).startsWith(text));
const openButtons = (wrapper) => buttonsWith(wrapper, OPEN_MEMBERS);

// A row of the networks list: the nearest ancestor of an "Open Members" button that holds no other such button.
function rowsOf(wrapper) {
  return openButtons(wrapper).map((button) => {
    let row = button.element;
    while (row.parentElement && row.parentElement.querySelectorAll("a, button").length > 0 && countOpen(row.parentElement) === 1) row = row.parentElement;
    return { text: () => row.textContent, element: row, find: (sel) => row.querySelector(sel) };
  });
}

const countOpen = (el) => [...el.querySelectorAll("a, button")].filter((b) => b.textContent.trim() === OPEN_MEMBERS).length;

const dotOf = (row) => row.find('[class*="dot"]')?.className ?? "";

// Where an "Open Members" link or chip leads: its `href`, or the hash after a click when it is a button.
async function targetOf(wrapperEl) {
  const href = wrapperEl.attributes("href");
  if (href) return href;
  await wrapperEl.trigger("click");
  await settle();
  return location.hash;
}
