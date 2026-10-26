/* The setup's first step: "Create a new network" opens a create form with no choice of type, and the request carries the name only. */

import { nextTick } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { baseState, mountSection } from "../testing/section.mjs";
import SetupWizard from "./SetupWizard.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

let mounted;

beforeEach(() => {
  localStorage.clear();
  api.mockReset();
  api.mockResolvedValue({ key: "mesh2-secret", label: "work", restart: false });
});
afterEach(() => mounted.wrapper.unmount());

describe("SetupWizard > step 1, no network > creating a network", () => {
  it("the first screen offers no type", () => {
    const { wrapper } = mountWizard();
    expect(wrapper.find("[id^=kind-]").exists()).toBe(false);
  });

  it("the create form has no type picker", async () => {
    const { wrapper } = await createOpened();
    expect(wrapper.find("#create-box").exists()).toBe(true);
    expect(wrapper.find("[id^=kind-]").exists()).toBe(false);
  });

  it("creates without a type: the request carries the name only", async () => {
    const { wrapper } = await createOpened();
    await wrapper.get("#create-label-input").setValue("work");
    await wrapper.get("#create-btn").trigger("click");
    await flushPromises();
    expect(api.mock.calls.filter((c) => c[1] === "/api/network/create").map((c) => c[2])).toEqual([{ label: "work" }]);
  });

  it("the join form has no type picker either", async () => {
    const { wrapper } = mountWizard();
    await wrapper.get("#home-start-join-btn").trigger("click");
    await nextTick();
    expect(wrapper.find("#join-box [id^=kind-]").exists()).toBe(false);
  });
});

async function createOpened() {
  const res = mountWizard();
  await res.wrapper.get("#home-start-create-btn").trigger("click");
  await nextTick();
  return res;
}

function mountWizard() {
  return (mounted = mountSection(SetupWizard, { state: baseState({ network: null, peers: [] }) }));
}
