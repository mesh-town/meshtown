/* Remembered exits on a node with several networks: a remembered id that the network no longer offers (a member that stopped
 * being an exit, or an id written into the wrong network) is not drawn, and a group without exits says so. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { storeSlotConfig } from "../core/config-meta.ts";
import { setRememberedFor } from "../common/exits.ts";
import { acceptOverview } from "../core/overview.ts";
import { createStore, STORE_KEY } from "../core/store.ts";
import { baseConfig, baseState } from "../testing/section.mjs";
import ExitCard from "./ExitCard.vue";
import { NETS_EXIT_NONE_IN } from "./nets-texts.ts";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const NETS = [["main", "friends"], ["k7q2xa", "my devices"], ["m4w5pz", "work"]];
const OFFER = { main: "D".repeat(64), k7q2xa: "E".repeat(64), m4w5pz: "F".repeat(64) };
const FORMER = "H".repeat(64);
const FOREIGN = "I".repeat(64);

let mounted;

beforeEach(() => localStorage.clear());
afterEach(() => mounted?.unmount());

const peerOf = (full, exit) => ({ id: full.slice(0, 8).toLowerCase(), id_full: full, name: "m-" + full[0].toLowerCase(), country: null, version: null, online: true, path: "direct", rtt_ms: 20, exit });

function stateOf(slot, peers) {
  const me = baseState().peers.find((p) => p.self);
  return baseState({ network: { id_hex: "hex-" + slot, label: slot, masked: "mesh1-••••-••••" }, peers: [me, ...peers], route: { state: "direct", exits: [] }, config_rev: 1 });
}

function mountCard(peersOf) {
  const store = createStore();
  const states = NETS.map(([slot]) => stateOf(slot, peersOf(slot)));
  acceptOverview(store, {
    api: 2,
    nets: NETS.map(([id, label], i) => ({ id, label, id_hex: "hex-" + id, status: "online", exit_enabled: false, proxy: id === "main", route_state: "direct", exits: [], members_voting: 0, joining: false, members_admitting: 0, me_excluded: false, me_pending: false, peers_online: 1, peers_total: 1, state: states[i] })),
    slots: NETS.map(([slot], i) => ({ slot, state: states[i], members: null })),
    restart_slots: [],
  });
  NETS.forEach(([slot]) => {
    const config = baseConfig({ route: { exits: [] } });
    if (slot === "main") Object.assign(store, { config, configRev: 1 });
    else storeSlotConfig(store, slot, { rev: 1, config }, "boot-1");
  });
  mounted = mount(ExitCard, { attachTo: document.body, global: { provide: { [STORE_KEY]: store } } });
  return mounted;
}

const idsIn = (wrapper, slot) => wrapper.findAll(`input.exit-choice[data-slot="${slot}"]`).map((i) => i.attributes("data-id"));
const noneLines = (wrapper) => wrapper.text().split(NETS_EXIT_NONE_IN).length - 1;

describe("a stale remembered exit", () => {
  it("of a former exit is not drawn and the group says that it has no exits", () => {
    setRememberedFor("m4w5pz", [FORMER]);
    const wrapper = mountCard((slot) => (slot === "m4w5pz" ? [peerOf(FORMER, false)] : [peerOf(OFFER[slot], true)]));
    expect(idsIn(wrapper, "m4w5pz")).toEqual([]);
    expect(noneLines(wrapper)).toBe(1);
    expect(wrapper.text()).not.toContain("member " + FORMER.slice(0, 10));
    expect(wrapper.text()).not.toContain("offline");
  });

  it("written into main from another network is not drawn there", () => {
    setRememberedFor("main", [FOREIGN]);
    const wrapper = mountCard((slot) => [peerOf(OFFER[slot], true)]);
    expect(idsIn(wrapper, "main")).toEqual([OFFER.main]);
    expect(wrapper.text()).not.toContain("offline");
  });

  it("of a member that still offers an exit stays", () => {
    setRememberedFor("k7q2xa", [OFFER.k7q2xa]);
    const wrapper = mountCard((slot) => [peerOf(OFFER[slot], true)]);
    expect(idsIn(wrapper, "k7q2xa")).toEqual([OFFER.k7q2xa]);
  });
});
