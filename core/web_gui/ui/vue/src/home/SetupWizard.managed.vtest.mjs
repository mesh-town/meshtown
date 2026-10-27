/* The join screen: "Create a new network" opens a create form with no choice of type, and the request carries the name only. */

import { nextTick } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { mountWizard, step, unmountWizard } from "../testing/wizard.mjs";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

let mounted;

beforeEach(() => {
  api.mockReset();
  api.mockResolvedValue({ key: "mesh2-secret", label: "work", restart: false });
});
afterEach(() => unmountWizard(mounted));

describe("SetupWizard > join screen > creating a network", () => {
  it("the first screen offers no type", () => {
    const { wrapper } = mountNoNetwork();
    expect(step(wrapper)).toBe("join");
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
    const { wrapper } = mountNoNetwork();
    await wrapper.get("#home-start-join-btn").trigger("click");
    await nextTick();
    expect(wrapper.find("#join-box [id^=kind-]").exists()).toBe(false);
  });
});

async function createOpened() {
  const res = mountNoNetwork();
  await res.wrapper.get("#home-start-create-btn").trigger("click");
  await nextTick();
  return res;
}

function mountNoNetwork() {
  return (mounted = mountWizard({ hasNet: false }));
}
