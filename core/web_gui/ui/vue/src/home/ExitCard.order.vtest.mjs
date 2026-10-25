import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { acceptOverview } from "../core/overview.ts";
import { createStore, STORE_KEY } from "../core/store.ts";
import { baseConfig, baseState } from "../testing/section.mjs";
import ExitCard from "./ExitCard.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const IDS = ["A".repeat(64), "B".repeat(64), "C".repeat(64)];
let mounted;

beforeEach(() => {
  localStorage.clear();
  api.mockImplementation(async (method, path, body) => ({ config: body?.config, rev: 2 }));
});
afterEach(() => mounted?.unmount());

function mountCard() {
  const store = createStore();
  const me = baseState().peers.find((p) => p.self);
  const peers = IDS.map((full, i) => ({ id: full.slice(0, 8).toLowerCase(), id_full: full, name: "m" + i, country: null, version: null, online: true, path: "direct", rtt_ms: 20, exit: true }));
  const state = baseState({ network: { id_hex: "hex-main", label: "main", masked: "mesh1-••••-••••" }, peers: [me, ...peers], route: { state: "direct", exits: [] }, config_rev: 1 });
  acceptOverview(store, {
    api: 2,
    nets: [{ id: "main", label: "main", id_hex: "hex-main", status: "online", exit_enabled: false, proxy: true, route_state: "direct", exits: [], members_voting: 0, joining: false, members_admitting: 0, me_excluded: false, me_pending: false, peers_online: 1, peers_total: 1, state }],
    slots: [{ slot: "main", state, members: null }],
    restart_slots: [],
  });
  Object.assign(store, { config: baseConfig({ route: { exits: [] } }), configRev: 1 });
  mounted = mount(ExitCard, { attachTo: document.body, global: { provide: { [STORE_KEY]: store } } });
  return mounted;
}

const order = (wrapper) => wrapper.findAll("input.exit-choice").map((i) => i.attributes("data-id"));
const flush = () => new Promise((r) => setTimeout(r, 0));

describe("the order of the exit rows", () => {
  it("does not change after a click on any row or on \"Directly\"", async () => {
    const wrapper = mountCard();
    const before = order(wrapper);
    expect(before).toEqual(IDS);
    for (const id of [IDS[2], IDS[0], IDS[1]]) {
      await wrapper.get(`input.exit-choice[data-id="${id}"]`).setValue(true);
      await flush();
      expect(order(wrapper)).toEqual(before);
    }
    await wrapper.get("#exit-direct").setValue(true);
    await flush();
    expect(order(wrapper)).toEqual(before);
  });
});
