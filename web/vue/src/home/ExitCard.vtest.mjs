/* "Exit via…": the list of exits, an empty list, a site check. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { baseState, mountSection, PEER_FULL } from "../testing/section.mjs";
import ExitCard from "./ExitCard.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

let mounted;

beforeEach(() => localStorage.clear());
afterEach(() => mounted.wrapper.unmount());

describe("ExitCard", () => {
  it("region \"Exit via…\", \"As usual\" and a member who allowed exiting", () => {
    const { wrapper } = mountExit();
    const region = wrapper.get("#home-exit");
    expect(region.attributes("role")).toBe("region");
    expect(wrapper.get("#home-exit-title").text()).toBe("Exit via…");
    expect(wrapper.get("#exit-direct").element.checked).toBe(true);
    expect(wrapper.findAll("input.exit-choice").map((i) => i.attributes("data-id"))).toEqual([PEER_FULL]);
  });

  it("the paragraph about \"exit\" is visible in the normal mode, in bare and with an empty list", () => {
    const phrase = "An exit is a member whose internet you use to open sites: sites see their address, not yours.";
    expect(mountExit().wrapper.get("#exit-explain").text()).toBe(phrase);
    mounted.wrapper.unmount();
    const peers = baseState().peers.map((p) => ({ ...p, exit: false }));
    const empty = mountExit(baseState({ peers })).wrapper.get("#exit-explain");
    expect(empty.text()).toBe(phrase);
    expect(empty.attributes("hidden")).toBeUndefined();
    mounted.wrapper.unmount();
    const bare = (mounted = mountSection(ExitCard, { state: baseState(), props: { bare: true } })).wrapper;
    expect(bare.get("#exit-explain").text()).toBe(phrase);
    expect(bare.find("#home-exit-title").exists()).toBe(false);
  });

  it("nobody allowed exiting — a hint instead of the list, the general hint hidden", () => {
    const peers = baseState().peers.map((p) => ({ ...p, exit: false }));
    const { wrapper } = mountExit(baseState({ peers }));
    expect(wrapper.text()).toContain("No other member offers their connection as an exit yet");
    expect(wrapper.findAll("input.exit-choice")).toHaveLength(0);
    expect(wrapper.get("#exit-list-hint").attributes("hidden")).toBeDefined();
  });

  it("in a network with no members found yet — \"Looking for other members\" instead of the empty hint", () => {
    const self = baseState().peers.filter((p) => p.self);
    const { wrapper } = mountExit(baseState({ peers: self }));
    expect(wrapper.get("#exit-searching").text()).toContain("Looking for other members");
    expect(wrapper.get("#exit-searching .spinner").exists()).toBe(true);
    expect(wrapper.text()).not.toContain("No other member offers");
  });

  it("members known but none offers exit — no searching text", () => {
    const peers = baseState().peers.map((p) => ({ ...p, exit: false }));
    const { wrapper } = mountExit(baseState({ peers }));
    expect(wrapper.find("#exit-searching").exists()).toBe(false);
  });

  it("choice status — role=status and empty until a choice is made", () => {
    const { wrapper } = mountExit();
    expect(wrapper.get("#exit-list-status").attributes("role")).toBe("status");
    expect(wrapper.get("#exit-list-status").attributes("hidden")).toBeDefined();
  });

  it("there is a network — the \"Check site\" field", () => {
    const { wrapper } = mountExit();
    expect(wrapper.find("#find-exit-input").exists()).toBe(true);
    expect(wrapper.find("#find-exit-no-network").exists()).toBe(false);
  });

  it("no network — a hint \"Join a Mesh Town network\" instead of the field", () => {
    const { wrapper } = mountExit(baseState({ network: null, peers: [] }));
    expect(wrapper.find("#find-exit-input").exists()).toBe(false);
    expect(wrapper.get("#find-exit-no-network").text()).toContain("Join a Mesh Town network");
  });

  it("empty input — \"Enter a site address…\", no request", async () => {
    const { wrapper } = mountExit();
    await wrapper.get("#find-exit-btn").trigger("click");
    expect(wrapper.get("#find-exit-result").text()).toBe("Enter a site address, for example example.com");
    expect(api).not.toHaveBeenCalled();
  });

  it("the search field hints \"for example, example.com\"", () => {
    expect(mountExit().wrapper.get("#find-exit-input").attributes("placeholder")).toBe("for example, example.com");
  });

  it("check failure — \"Couldn't check the site: …\"", async () => {
    api.mockRejectedValue(new Error("Node is not responding"));
    const { wrapper } = mountExit();
    await wrapper.get("#find-exit-input").setValue("example.com");
    await wrapper.get("#find-exit-btn").trigger("click");
    await flushPromises();
    expect(wrapper.get("#find-exit-result").text()).toBe("Couldn't check the site: node is not responding");
  });

  it("the site check is not called without a click", () => {
    mountExit();
    expect(api).not.toHaveBeenCalled();
  });
});

function mountExit(state = baseState()) {
  return (mounted = mountSection(ExitCard, { state }));
}
