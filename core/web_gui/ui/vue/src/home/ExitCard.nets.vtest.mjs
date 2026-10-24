/* "Exit via…" on a node with several networks: one group of exits per network that holds a key, the choice is written
 * into the network of the clicked exit, only the exit in the network that carries the browser proxy is checked, "Directly"
 * hands the proxy back to the primary network. One network keeps the former markup and bare ids as values. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { storeSlotConfig } from "../core/config-meta.ts";
import { acceptOverview } from "../core/overview.ts";
import { createStore, STORE_KEY } from "../core/store.ts";
import { baseConfig, baseState } from "../testing/section.mjs";
import ExitCard from "./ExitCard.vue";
import { NETS_EXIT_NONE_IN, NETS_EXIT_NO_NET } from "./nets-texts.ts";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const NET_A = "main";
const NET_B = "k7q2xa";
const NET_C = "m4w5pz";
const EXIT_A = "D".repeat(64);
const EXIT_B = "E".repeat(64);
const EXIT_C = "F".repeat(64);
const SHARED = "G".repeat(64); // one member that sits in two networks

let mounted;

beforeEach(() => localStorage.clear());
afterEach(() => mounted?.wrapper.unmount());

describe("ExitCard with several networks", () => {
  describe("three networks with a key, each with its own exit members", () => {
    it("draws one titled group per network, named after the network", () => {
      const { wrapper } = setup(threeNets());
      expect(titles(wrapper)).toEqual(["friends", "my devices", "work"]);
      expect(wrapper.findAll(".exit-group-title").map((h) => h.attributes("data-slot"))).toEqual([NET_A, NET_B, NET_C]);
    });

    it("lists in each group only the exits of that network", () => {
      const { wrapper } = setup(threeNets());
      expect(idsIn(wrapper, NET_A)).toEqual([EXIT_A]);
      expect(idsIn(wrapper, NET_B)).toEqual([EXIT_B]);
      expect(idsIn(wrapper, NET_C)).toEqual([EXIT_C]);
    });

    it("gives every exit row a value that names its network", () => {
      const { wrapper } = setup(threeNets());
      expect(radio(wrapper, NET_B, EXIT_B).element.value).toBe(NET_B + "|" + EXIT_B);
      expect(radio(wrapper, NET_A, EXIT_A).element.value).toBe(NET_A + "|" + EXIT_A);
    });
  });

  describe("the browser proxy points to one network", () => {
    it("checks the exit of that network and no other row", () => {
      const { wrapper } = setup(threeNets({ proxy: NET_B, chosen: NET_B }));
      expect(checkedRows(wrapper)).toEqual([NET_B + "|" + EXIT_B]);
    });

    it("keeps \"Directly\" unchecked while an exit is checked", () => {
      const { wrapper } = setup(threeNets({ proxy: NET_B, chosen: NET_B }));
      expect(wrapper.get("#exit-direct").element.checked).toBe(false);
    });

    it("checks the member only in the proxy network when it sits in two networks", () => {
      const { wrapper } = setup(sharedMember(NET_B));
      expect(radio(wrapper, NET_B, SHARED).element.checked, "row in the proxy network").toBe(true);
      expect(radio(wrapper, NET_A, SHARED).element.checked, "same id in a network without the proxy").toBe(false);
    });

    it("checks the member in the primary network when the proxy is there", () => {
      const { wrapper } = setup(sharedMember(NET_A));
      expect(radio(wrapper, NET_A, SHARED).element.checked, "row in the proxy network").toBe(true);
      expect(radio(wrapper, NET_B, SHARED).element.checked, "same id in a network without the proxy").toBe(false);
    });
  });

  describe("an exit of another network is clicked while the proxy is in the primary one", () => {
    it("writes the exit into the config of the clicked network", async () => {
      const { wrapper, writes } = setup(threeNets({ proxy: NET_A, chosen: NET_A }));
      await radio(wrapper, NET_B, EXIT_B).setValue();
      await settled();
      expect(writes().map((w) => w.path)).toEqual(["/api/net/" + NET_B + "/config"]);
      expect(writes()[0].body.config.route.exits).toEqual([EXIT_B]);
    });

    it("leaves the primary config untouched", async () => {
      const { wrapper, writes } = setup(threeNets({ proxy: NET_A, chosen: NET_A }));
      await radio(wrapper, NET_C, EXIT_C).setValue();
      await settled();
      expect(writes().filter((w) => w.path === "/api/config")).toEqual([]);
    });
  });

  describe("an exit of the primary network is clicked while the proxy is in another one", () => {
    it("writes the exit into the primary config", async () => {
      const { wrapper, writes } = setup(threeNets({ proxy: NET_B, chosen: NET_B }));
      await radio(wrapper, NET_A, EXIT_A).setValue();
      await settled();
      expect(writes().map((w) => w.path)).toEqual(["/api/config"]);
      expect(writes()[0].body.config.route.exits).toEqual([EXIT_A]);
    });
  });

  describe("\"Directly\" is clicked", () => {
    it("with the proxy in another network, clears the exits and gives the proxy back to the primary one in one request", async () => {
      const { wrapper, writes } = setup(threeNets({ proxy: NET_B, chosen: NET_B }));
      await wrapper.get("#exit-direct").setValue();
      await settled();
      expect(writes().map((w) => w.path)).toEqual(["/api/config"]);
      const config = writes()[0].body.config;
      expect(config.route.exits).toEqual([]);
      expect(config.proxy.net).toBe(NET_A);
    });

    it("with the proxy in the primary network, only clears the exits", async () => {
      const { wrapper, writes } = setup(threeNets({ proxy: NET_A, chosen: NET_A }));
      await wrapper.get("#exit-direct").setValue();
      await settled();
      expect(writes().map((w) => w.path)).toEqual(["/api/config"]);
      const config = writes()[0].body.config;
      expect(config.route.exits).toEqual([]);
      expect(config.proxy, "the proxy network is not touched").toBeUndefined();
    });
  });

  describe("no network carries the proxy", () => {
    it("says the network of the chosen exit is gone", () => {
      const { wrapper } = setup(threeNets({ proxy: null, chosen: NET_B }));
      expect(wrapper.text()).toContain(NETS_EXIT_NO_NET);
    });

    it("checks none of the exit rows, even though a network lists an exit", () => {
      const { wrapper } = setup(threeNets({ proxy: null, chosen: NET_B }));
      expect(checkedRows(wrapper)).toEqual([]);
    });

    it("does not say it when some network carries the proxy", () => {
      const { wrapper } = setup(threeNets({ proxy: NET_B, chosen: NET_B }));
      expect(wrapper.text()).not.toContain(NETS_EXIT_NO_NET);
    });
  });

  describe("a network has no exit members", () => {
    it("shows the \"none in this network\" line under its title, between the neighbour groups", () => {
      const { wrapper } = setup(threeNets({ offers: { [NET_B]: [] } }));
      expect(titles(wrapper)).toEqual(["friends", "my devices", "work"]);
      const line = lineWith(wrapper, NETS_EXIT_NONE_IN);
      expect(follows(titleOf(wrapper, NET_B), line), "after the title of its network").toBe(true);
      expect(follows(line, titleOf(wrapper, NET_C)), "before the next group").toBe(true);
    });

    it("shows the line once: the other groups have exits", () => {
      const { wrapper } = setup(threeNets({ offers: { [NET_B]: [] } }));
      expect(wrapper.text().split(NETS_EXIT_NONE_IN)).toHaveLength(2);
    });
  });
});

describe("ExitCard with one network", () => {
  it("draws no group titles", () => {
    const { wrapper } = setup(oneNet());
    expect(wrapper.findAll(".exit-group-title")).toHaveLength(0);
  });

  it("uses the bare member id as the value of the row", () => {
    const { wrapper } = setup(oneNet());
    expect(wrapper.findAll("input.exit-choice").map((i) => [i.attributes("data-id"), i.element.value])).toEqual([[EXIT_A, EXIT_A]]);
  });

  it("checks the chosen exit by its bare id", () => {
    const { wrapper } = setup(oneNet(true));
    expect(checkedRows(wrapper)).toEqual([EXIT_A]);
  });

  it("writes the choice into the primary config", async () => {
    const { wrapper, writes } = setup(oneNet());
    await wrapper.get("input.exit-choice").setValue();
    await settled();
    expect(writes().map((w) => w.path)).toEqual(["/api/config"]);
    expect(writes()[0].body.config.route.exits).toEqual([EXIT_A]);
  });

  it("says nothing about a missing proxy network", () => {
    expect(setup(oneNet()).wrapper.text()).not.toContain(NETS_EXIT_NO_NET);
  });
});

// A node with the networks `NET_A` (primary), `NET_B`, `NET_C`. `proxy` — the slot whose brief carries `proxy: true`
// (`null` — none); `chosen` — the slot whose route holds its exit (others hold none); `offers` — per-slot exit members.
function threeNets({ proxy = NET_A, chosen = null, offers = {} } = {}) {
  const own = { [NET_A]: [EXIT_A], [NET_B]: [EXIT_B], [NET_C]: [EXIT_C], ...offers };
  const label = { [NET_A]: "friends", [NET_B]: "my devices", [NET_C]: "work" };
  return [NET_A, NET_B, NET_C].map((id) => ({ id, label: label[id], offers: own[id], exits: id === chosen ? own[id].slice(0, 1) : [], proxy: id === proxy }));
}

// The same member `SHARED` is an exit in the primary network and in `NET_B`; both routes list it, only `proxy` carries the proxy.
function sharedMember(proxy) {
  const both = (id, label) => ({ id, label, offers: [SHARED], exits: [SHARED], proxy: id === proxy });
  return [both(NET_A, "friends"), both(NET_B, "my devices")];
}

function oneNet(chosen = false) {
  return [{ id: NET_A, label: "friends", offers: [EXIT_A], exits: chosen ? [EXIT_A] : [], proxy: true }];
}

// Mounts the card on a store filled as the overview does it; `PUT` answers with the written document.
function setup(specs) {
  api.mockReset();
  api.mockImplementation(async (method, path, body) => ({ rev: (body?.rev ?? 0) + 1, config: body?.config, read_only: [], restart_fields: [], restart_pending: [], cli_overrides: [] }));
  const store = storeOf(specs);
  const wrapper = mount(ExitCard, { attachTo: document.body, global: { provide: { [STORE_KEY]: store } } });
  mounted = { wrapper, store };
  const writes = () => api.mock.calls.filter(([method]) => method === "PUT").map(([, path, body]) => ({ path, body }));
  return { wrapper, store, writes };
}

function storeOf(specs) {
  const store = createStore();
  const states = specs.map(stateOfSpec);
  acceptOverview(store, {
    api: 2,
    nets: specs.map((s, i) => briefOf(s, states[i])),
    slots: specs.map((s, i) => ({ slot: s.id, state: states[i], members: null })),
    restart_slots: [],
  });
  specs.forEach((s) => {
    const config = baseConfig({ route: { exits: s.exits } });
    if (s.id === NET_A) Object.assign(store, { config, configRev: 1 });
    else storeSlotConfig(store, s.id, { rev: 1, config }, "boot-1");
  });
  return store;
}

const peerOf = (full) => ({ id: full.slice(0, 8).toLowerCase(), id_full: full, name: "exit-" + full[0].toLowerCase(), country: null, version: null, online: true, path: "direct", rtt_ms: 20, exit: true });
const chosenOf = (full) => ({ id: full.slice(0, 8).toLowerCase(), id_full: full, name: "exit-" + full[0].toLowerCase(), country: null, exit: true, online: true, active: true });

function stateOfSpec(spec) {
  const me = baseState().peers.find((p) => p.self);
  return baseState({
    network: { id_hex: "hex-" + spec.id, label: spec.label, masked: "mesh1-••••-••••" },
    peers: [me, ...spec.offers.map(peerOf)],
    route: { state: spec.exits.length ? "exit" : "direct", exits: spec.exits.map(chosenOf) },
    config_rev: 1,
  });
}

function briefOf(spec, state) {
  return {
    id: spec.id, label: spec.label, id_hex: "hex-" + spec.id, status: "online", exit_enabled: false, proxy: spec.proxy,
    route_state: state.route.state, exits: spec.exits, members_voting: 0, joining: false, members_admitting: 0,
    me_excluded: false, me_pending: false, peers_online: spec.offers.length, peers_total: spec.offers.length,
  };
}

const settled = () => flushPromises();
const titles = (wrapper) => wrapper.findAll(".exit-group-title").map((h) => h.text());
const titleOf = (wrapper, slot) => wrapper.get(`.exit-group-title[data-slot="${slot}"]`).element;
const idsIn = (wrapper, slot) => wrapper.findAll(`input.exit-choice[data-slot="${slot}"]`).map((i) => i.attributes("data-id"));
const radio = (wrapper, slot, id) => wrapper.get(`input.exit-choice[data-slot="${slot}"][data-id="${id}"]`);
const checkedRows = (wrapper) => wrapper.findAll("input.exit-choice").filter((i) => i.element.checked).map((i) => i.element.value);

// The element of the exit list whose own text is `text`.
function lineWith(wrapper, text) {
  const hit = [...wrapper.get("#exit-list").element.querySelectorAll("*")].filter((el) => el.children.length === 0 && el.textContent.trim() === text);
  expect(hit, `one element with the text "${text}"`).toHaveLength(1);
  return hit[0];
}

// `later` comes after `earlier` in the document.
const follows = (earlier, later) => Boolean(earlier.compareDocumentPosition(later) & Node.DOCUMENT_POSITION_FOLLOWING);
