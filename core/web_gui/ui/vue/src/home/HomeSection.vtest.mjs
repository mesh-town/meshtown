/* The "Home" island: without a network — the setup with a join card, with a network — the status panel and the exit choice. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { ApiError, api } from "../core/api.ts";
import { apiTable, baseConfig, baseState, mountSection, PEER_FULL } from "../testing/section.mjs";
import HomeSection from "./HomeSection.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const PANEL = '[aria-label="How sites open right now"]';
const putConfig = (body) => ({ config: body.config, rev: 2 });
let mounted;

beforeEach(() => localStorage.clear());
afterEach(() => mounted.wrapper.unmount());

describe("HomeSection > no network", () => {
  it("the \"You haven't joined\" panel, two buttons, no form before a click, the join screen has no navigation, no status panel", () => {
    const { wrapper } = mountHome({ state: baseState({ network: null, peers: [] }) });
    expect(wrapper.find("#join-card").exists()).toBe(false);
    expect(wrapper.get("#home-start-title").text()).toBe("You haven't joined a Mesh Town network yet");
    expect(wrapper.get("#home-start-join-btn").text()).toBe("Join with an invite");
    expect(wrapper.get("#home-start-create-btn").text()).toBe("Create a new network");
    expect(wrapper.get("#wizard-screen").attributes("data-step")).toBe("join");
    expect(wrapper.find("#wizard-back").exists(), "no Back on the first screen").toBe(false);
    expect(wrapper.find("#wizard-next").exists(), "the join screen has its own buttons").toBe(false);
    expect(wrapper.find(PANEL).exists()).toBe(false);
  });
});

describe("HomeSection > there is a network, sites open as usual", () => {
  it("the region \"How sites open right now\" says \"directly\", no setup", () => {
    const { wrapper } = mountHome({ state: directState() });
    expect(wrapper.get(PANEL).attributes("role")).toBe("region");
    expect(wrapper.get("#route-state").text()).toContain("directly");
    expect(wrapper.find(".wizard-card").exists()).toBe(false);
  });

  it("a click on a member — PUT /api/config, status \"Saved\"", async () => {
    api.mockImplementation(apiTable({ "PUT /api/config": putConfig }));
    const { wrapper } = mountHome({ state: directState() });
    await wrapper.get(`input[data-id="${PEER_FULL}"]`).setValue(true);
    await flushPromises();
    const [method, path, body] = api.mock.calls[0];
    expect([method, path]).toEqual(["PUT", "/api/config"]);
    expect(body.config.route.exits).toEqual([PEER_FULL]);
    expect(wrapper.get("#exit-list-status").text()).toBe("Saved");
    expect(wrapper.get("#exit-list-status").attributes("role")).toBe("status");
  });

  it("node refused — the \"As usual\" radio is marked again, the member is cleared", async () => {
    api.mockRejectedValue(new ApiError(422, "invalid", "refused", { fields: [{ path: "route.exits", message: "Exit unavailable" }] }));
    const { wrapper } = mountHome({ state: directState() });
    await wrapper.get(`input[data-id="${PEER_FULL}"]`).setValue(true);
    await flushPromises();
    expect(wrapper.get("#exit-list-status").text()).toBe("Exit unavailable");
    expect(wrapper.get("#exit-direct").element.checked, "selection rolled back: v-model, not :checked").toBe(true);
    expect(wrapper.get(`input[data-id="${PEER_FULL}"]`).element.checked).toBe(false);
  });
});

describe("HomeSection > an exit is chosen, \"As usual\" is rejected by the node", () => {
  it("node refused \"As usual\" — the member is marked again, \"As usual\" is cleared", async () => {
    api.mockRejectedValue(new ApiError(422, "invalid", "refused", { fields: [{ path: "route.exits", message: "Not now" }] }));
    const state = baseState({ route: { state: "exit", exits: [{ id_full: PEER_FULL, active: true }] }, config_rev: 1 });
    const { wrapper } = mountHome({ state, config: baseConfig({ route: { exits: [PEER_FULL] } }) });
    await wrapper.get("#exit-direct").setValue(true);
    await flushPromises();
    expect(wrapper.get(`input[data-id="${PEER_FULL}"]`).element.checked).toBe(true);
    expect(wrapper.get("#exit-direct").element.checked).toBe(false);
    expect(wrapper.get("#exit-list-status").text()).toBe("Not now");
  });
});

describe("HomeSection > no snapshot yet", () => {
  it("a single line \"Loading…\"", () => {
    const { wrapper } = mountHome({ state: null });
    expect(wrapper.text()).toContain("Loading…");
    expect(wrapper.find("#join-card").exists()).toBe(false);
  });
});

function mountHome(detail) {
  return (mounted = mountSection(HomeSection, { config: baseConfig(), ...detail }));
}

// There is a network, the route is direct, member peer-one allowed exiting through them.
function directState() {
  return baseState({ route: { state: "direct", exits: [] }, config_rev: 1 });
}
