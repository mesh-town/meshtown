/* Join progress by invite on "Home": the setup (a progress block instead of buttons) and "Main" (the status panel). */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { baseState, mountSection } from "../testing/section.mjs";
import HomeSection from "./HomeSection.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const INFO = { state: "waiting", message: "waiting", label: "family", expires_unix: 1 };
const lonely = (invite_join) => baseState({ network: null, peers: [], node: { ...baseState().node, invite_join } });
let mounted;

beforeEach(() => { localStorage.clear(); api.mockReset(); });
afterEach(() => mounted.wrapper.unmount());

describe("HomeSection > join progress, setup", () => {
  it("waiting: the progress block role=status, no join buttons, step 1 — \"Looking for the network…\"", () => {
    const { wrapper } = mountHome(lonely(INFO));
    const block = wrapper.get(".join-progress");
    expect(block.attributes("role")).toBe("status");
    expect(block.attributes("aria-live")).toBe("polite");
    expect(block.get(".spinner").attributes("aria-hidden")).toBe("true");
    expect(block.get(".join-progress-main").text()).toBe("Looking for the network \"family\"…");
    expect(wrapper.find("#home-start-join-btn").exists()).toBe(false);
    expect(wrapper.find("#join-card").exists()).toBe(false);
    expect(wrapper.get(".wizard-step .step-head").text()).toBe("Looking for the network…");
    expect(wrapper.findAll(".wizard-step")[0].find(".step-num").exists()).toBe(false);
  });

  it("refused: the error text once and the join buttons are back", () => {
    const { wrapper } = mountHome(lonely({ ...INFO, state: "refused", message: "The invite has already been used." }));
    expect(wrapper.find(".join-progress").exists()).toBe(false);
    expect(wrapper.get("#join-refused").text()).toBe("Couldn't join \"family\". The invite has already been used.");
    expect(wrapper.find("#home-start-join-btn").exists()).toBe(true);
    expect(wrapper.text().split("The invite has already been used.").length - 1).toBe(1);
  });

  it("joined reply: \"Joining the network…\" while the snapshot is old; a new start clears the block", async () => {
    api.mockResolvedValue({ joined: true, label: "family" });
    const { wrapper, store } = mountHome(lonely(null));
    await wrapper.get("#home-start-join-btn").trigger("click");
    await wrapper.get("#join-key-input").setValue("mesh-invite2-abc");
    await wrapper.get("#join-btn").trigger("click");
    await flushPromises();
    expect(wrapper.get(".join-progress-main").text()).toBe("Joining the network…");
    expect(wrapper.get(".wizard-step .step-head").text()).toBe("Joining the network…");
    store.state = baseState({ boot_id: "boot-2" });
    await flushPromises();
    expect(wrapper.find(".join-progress").exists()).toBe(false);
  });
});

describe("HomeSection > join progress, \"Main\"", () => {
  it("waiting: the status panel is grey with a spinner and \"Looking for the network…\"", async () => {
    const { wrapper } = mountHome(lonely(INFO));
    await wrapper.get("#home-tab-main").trigger("click");
    expect(wrapper.get("#route-state").text()).toBe("Looking for the network \"family\"…");
    expect(wrapper.get(".state-panel .spinner").attributes("aria-hidden")).toBe("true");
    expect(wrapper.find(".state-panel .state-dot").exists()).toBe(false);
  });

  it("refused: the panel is red, no spinner", async () => {
    const { wrapper } = mountHome(lonely({ ...INFO, state: "refused", message: "Refused." }));
    await wrapper.get("#home-tab-main").trigger("click");
    expect(wrapper.get("#route-state").text()).toBe("Refused.");
    expect(wrapper.find(".state-panel .spinner").exists()).toBe(false);
    expect(wrapper.get(".state-panel").classes()).toContain("ring-level-red");
  });
});

function mountHome(state) {
  return (mounted = mountSection(HomeSection, { state }));
}
