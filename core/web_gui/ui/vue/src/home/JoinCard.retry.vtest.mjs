/* The join card: a new attempt clears the refusal shown before it; an empty field does not. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { baseState, mountSection } from "../testing/section.mjs";
import JoinCard from "./JoinCard.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const lonely = (invite_join) => baseState({ network: null, peers: [], node: { ...baseState().node, invite_join } });
const WAITING = { state: "waiting", message: "waiting", label: "family", expires_unix: 1 };
const REFUSED = { ...WAITING, state: "refused", message: "This invite cannot be used." };
let mounted;

beforeEach(() => { localStorage.clear(); api.mockReset(); });
afterEach(() => mounted.wrapper.unmount());

async function open() {
  api.mockResolvedValue({ waiting: true });
  mounted = mountSection(JoinCard, { state: lonely(REFUSED) });
  await flushPromises();
  return mounted;
}
const refusalShown = (wrapper) => wrapper.get("#join-wait").classes().includes("field-error-text") && wrapper.get("#join-wait").attributes("hidden") === undefined;

describe("JoinCard > a new attempt", () => {
  it("a click with an invite clears the old refusal", async () => {
    const { wrapper } = await open();
    expect(refusalShown(wrapper)).toBe(true);
    await wrapper.get("#join-key-input").setValue("mesh-invite2-abc");
    await wrapper.get("#join-btn").trigger("click");
    await flushPromises();
    expect(refusalShown(wrapper)).toBe(false);
    expect(wrapper.text()).not.toContain("This invite cannot be used.");
  });

  it("the same refusal after waiting shows again", async () => {
    const { wrapper, store } = await open();
    await wrapper.get("#join-key-input").setValue("mesh-invite2-abc");
    await wrapper.get("#join-btn").trigger("click");
    await flushPromises();
    store.state = lonely(WAITING);
    await flushPromises();
    expect(wrapper.get("#join-wait").text()).toContain("Waiting");
    store.state = lonely(REFUSED);
    await flushPromises();
    expect(wrapper.get("#join-wait").text()).toBe("Couldn't join \"family\". This invite cannot be used.");
  });

  it("an empty field: the refusal stays", async () => {
    const { wrapper } = await open();
    await wrapper.get("#join-btn").trigger("click");
    await flushPromises();
    expect(wrapper.get("#join-wait").text()).toBe("Couldn't join \"family\". This invite cannot be used.");
  });
});
