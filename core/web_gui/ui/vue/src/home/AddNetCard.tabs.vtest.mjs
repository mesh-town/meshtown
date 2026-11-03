/* The Join / Create tabs of the "Join or create another network" card: which one is active. */

import { mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../core/api.ts";
import { acceptOverview, parseOverview } from "../core/overview.ts";
import { STORE_KEY, createStore } from "../core/store.ts";
import { TOKEN_KEY } from "../core/token.ts";
import { createFakeNode } from "../testing/fake-node.mjs";
import { twoNets } from "../testing/overview-fixtures.mjs";
import { settle } from "../testing/page.mjs";
import AddNetCard from "./AddNetCard.vue";
import { resetNetBanner } from "./add-net-banner.ts";

let wrapper;

beforeEach(() => {
  sessionStorage.clear();
  location.hash = "";
  resetNetBanner();
});
afterEach(() => {
  wrapper.unmount();
  location.hash = "";
});

describe("the Join and Create tabs", () => {
  it("mark Join as active by default and Create after a click", async () => {
    await mountCard();
    expectActive("#add-net-tab-join", "#add-net-tab-create");
    await wrapper.get("#add-net-tab-create").trigger("click");
    await settle();
    expectActive("#add-net-tab-create", "#add-net-tab-join");
  });
});

function expectActive(on, off) {
  expect(wrapper.get(on).classes()).toContain("active");
  expect(wrapper.get(on).attributes("aria-selected")).toBe("true");
  expect(wrapper.get(off).classes()).not.toContain("active");
  expect(wrapper.get(off).attributes("aria-selected")).toBe("false");
}

async function mountCard() {
  const opts = twoNets();
  opts.slots = [...opts.slots, { id: "neta22" }];
  const node = createFakeNode(opts);
  vi.stubGlobal("fetch", node.fetch);
  sessionStorage.setItem(TOKEN_KEY, "test-token");
  const store = createStore();
  acceptOverview(store, parseOverview(await api("GET", "/api/overview")));
  wrapper = mount(AddNetCard, { attachTo: document.body, global: { provide: { [STORE_KEY]: store } } });
  await settle();
}
