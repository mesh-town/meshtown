/* "Exit via…": the chosen member stopped offering an exit — the meta says so and a hint stands under the row. */

import { afterEach, describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";
import { STORE_KEY } from "../core/store.ts";
import { netsStore } from "../testing/nets-store.mjs";
import ExitCard from "./ExitCard.vue";
import { NETS_EXIT_NOT_OFFERED } from "./nets-texts.ts";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const X = "D".repeat(64);
const Y = "E".repeat(64);
const HINT = ".exit-not-offered";
let wrapper;

afterEach(() => wrapper?.unmount());

const net = (id, offers, exits = [], proxy = true) => ({ id, label: id === "main" ? "friends" : "work", offers, exits, proxy });
const twoNets = (offers) => [net("main", offers, [X], true), net("k7q2xa", [Y], [], false)];

function show(specs, mutate) {
  const store = netsStore(specs);
  mutate?.(store);
  wrapper = mount(ExitCard, { attachTo: document.body, global: { provide: { [STORE_KEY]: store } } });
  return wrapper;
}
const metaOf = (id) => wrapper.get(`.exit-meta[data-id="${id}"]`).text();
const setPeer = (store, patch) => Object.assign(store.state.peers.find((p) => p.id_full === X), patch);

describe("ExitCard, the chosen exit is not an exit any more", () => {
  it("shows \"not an exit\" and the hint under the row", () => {
    show(twoNets([{ full: X, exit: false }]));
    expect(metaOf(X)).toBe("not an exit");
    expect(wrapper.get(HINT).text()).toBe(NETS_EXIT_NOT_OFFERED("exit-d"));
    expect(wrapper.get(HINT).attributes("data-slot")).toBe("main");
  });

  it("one network: same", () => {
    show([net("main", [{ full: X, exit: false }], [X])]);
    expect(metaOf(X)).toBe("not an exit");
    expect(wrapper.find(HINT).exists()).toBe(true);
  });

  it("offline wins over \"not an exit\": no hint", () => {
    show(twoNets([{ full: X, exit: false }]), (s) => setPeer(s, { online: false }));
    expect(metaOf(X)).toBe("offline");
    expect(wrapper.find(HINT).exists()).toBe(false);
  });

  it("blocked wins over \"not an exit\": no hint", () => {
    show(twoNets([{ full: X, exit: false }]), (s) => { s.members = { members: [{ id_full: X, blocked_by_me: true }] }; });
    expect(metaOf(X)).not.toBe("not an exit");
    expect(wrapper.find(HINT).exists()).toBe(false);
  });

  it("an offering exit has neither the mark nor the hint", () => {
    show(twoNets([X]));
    expect(metaOf(X)).toMatch(/^online/);
    expect(wrapper.find(HINT).exists()).toBe(false);
  });

  it("an unchosen non-offering member is not listed or marked", () => {
    show([net("main", [{ full: X, exit: false }], [])]);
    expect(wrapper.find(`.exit-meta[data-id="${X}"]`).exists()).toBe(false);
    expect(wrapper.find(HINT).exists()).toBe(false);
  });
});
