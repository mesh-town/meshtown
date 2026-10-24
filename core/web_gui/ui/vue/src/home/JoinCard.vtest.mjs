/* The join card: an empty field, a node error with a key word. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { ApiError, api } from "../core/api.ts";
import { baseState, mountSection } from "../testing/section.mjs";
import JoinCard from "./JoinCard.vue";
import SetupWizard from "./SetupWizard.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

let mounted;

beforeEach(() => localStorage.clear());
afterEach(() => mounted.wrapper.unmount());

describe("JoinCard", () => {
  it("empty \"Join\" — role=alert with text, api not called", async () => {
    const { wrapper } = await mountHomeJoin();
    await wrapper.get("#join-btn").trigger("click");
    await flushPromises();
    const alert = wrapper.get("#join-key-error");
    expect(alert.attributes("role")).toBe("alert");
    expect(alert.text()).toBe("Paste the invite first.");
    expect(api).not.toHaveBeenCalled();
  });

  it("422 with a word — the word in <mark>, the rest of the text next to it", async () => {
    const body = { message: "The word \"zebra\" isn't in the dictionary", word: "zebra" };
    api.mockRejectedValue(new ApiError(422, "invalid_key", "x", body));
    const { wrapper } = await mountHomeJoin();
    await wrapper.get("#join-key-input").setValue("mesh-invite2-abc");
    await wrapper.get("#join-btn").trigger("click");
    await flushPromises();
    expect(wrapper.get("#join-key-error mark").text()).toBe("\"zebra\"");
    expect(wrapper.get("#join-key-error").text()).toBe("The word \"zebra\" isn't in the dictionary");
  });

  it("<b> in the node's message stays text, not markup", async () => {
    api.mockRejectedValue(new ApiError(422, "invalid_key", "x", { message: "bad <b>bold</b>" }));
    const { wrapper } = await mountHomeJoin();
    await wrapper.get("#join-key-input").setValue("k");
    await wrapper.get("#join-btn").trigger("click");
    await flushPromises();
    expect(wrapper.find("#join-key-error b").exists()).toBe(false);
    expect(wrapper.get("#join-key-error").text()).toContain("<b>bold</b>");
  });

  it("#join-wait: empty at invite_join null, visible with text at waiting, a refusal — the node's message", async () => {
    const idle = mountJoin({ state: baseState({ network: null, peers: [] }) });
    expect(idle.wrapper.get("#join-wait").attributes("hidden")).toBeDefined();
    expect(idle.wrapper.get("#join-wait").text()).toBe("");
    idle.wrapper.unmount();
    const info = { state: "waiting", message: "waiting", label: "family", expires_unix: 1 };
    const node = { ...baseState().node, invite_join: info };
    const waiting = mountJoin({ state: baseState({ network: null, peers: [], node }) });
    const el = waiting.wrapper.get("#join-wait");
    expect(el.attributes("hidden")).toBeUndefined();
    expect(el.text()).toBe("Waiting for members of the network \"family\" to come online. As soon as one of the members from the invite is online, Mesh Town will join the network.");
    waiting.store.state.node.invite_join = { ...info, state: "refused", message: "Another node has already joined with this invite." };
    await waiting.wrapper.vm.$nextTick();
    expect(el.text()).toBe("Another node has already joined with this invite.");
  });

  it("with a network the title is \"Join with an invite\", the button is \"Join\"", () => {
    const { wrapper } = mountJoin({ state: baseState() });
    expect(wrapper.get("#join-card-title").text()).toBe("Join with an invite");
    expect(wrapper.get("#join-btn").text()).toBe("Join");
    expect(wrapper.get("#net-restart-hint").attributes("hidden")).toBeDefined();
  });

  it("without a network the restart hint is hidden", async () => {
    const { wrapper } = await mountHomeJoin();
    expect(wrapper.get("#net-restart-hint").attributes("hidden")).toBeDefined();
    expect(wrapper.get("#join-card-title").text()).toBe("Join with an invite");
  });

  it("with a network: the field is visible, \"Create a new network\" is there, a click expands creation", async () => {
    const { wrapper } = mountJoin({ state: baseState() });
    expect(wrapper.get("#join-box").attributes("hidden")).toBeUndefined();
    expect(wrapper.get("#create-box").attributes("hidden")).toBeDefined();
    await wrapper.get("#create-toggle").trigger("click");
    expect(wrapper.get("#create-box").attributes("hidden")).toBeUndefined();
  });

  it("with a network \"Create\" is not the primary button", () => {
    const { wrapper } = mountJoin({ state: baseState() });
    expect(wrapper.get("#create-btn").classes()).not.toContain("btn-primary");
  });
});

// Without a network the form is opened by the button of the "Home" panel, not by the card itself.
async function mountHomeJoin(detail = { state: baseState({ network: null, peers: [] }) }) {
  const res = (mounted = mountSection(SetupWizard, detail));
  await res.wrapper.get("#home-start-join-btn").trigger("click");
  return res;
}

function mountJoin(detail = { state: baseState({ network: null, peers: [] }) }) {
  return (mounted = mountSection(JoinCard, detail));
}
