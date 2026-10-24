/* The join card after a refusal: the stale "Invite sent" line goes away, the waiting line stays while waiting. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { baseState, mountSection } from "../testing/section.mjs";
import JoinCard from "./JoinCard.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const lonely = (invite_join) => baseState({ network: null, peers: [], node: { ...baseState().node, invite_join } });
const INFO = { state: "waiting", message: "waiting", label: "family", expires_unix: 1 };
const REFUSED = { ...INFO, state: "refused", message: "This invite cannot be used." };
let mounted;

beforeEach(() => { localStorage.clear(); api.mockReset(); });
afterEach(() => mounted.wrapper.unmount());

async function sendInvite() {
  api.mockResolvedValue({ waiting: true });
  mounted = mountSection(JoinCard, { state: lonely(null) });
  await mounted.wrapper.get("#join-key-input").setValue("mesh-invite2-abc");
  await mounted.wrapper.get("#join-btn").trigger("click");
  await flushPromises();
  return mounted;
}

describe("JoinCard > invite_join", () => {
  it("waiting: the \"Invite sent\" line is shown", async () => {
    const { wrapper, store } = await sendInvite();
    store.state = lonely(INFO);
    await flushPromises();
    expect(wrapper.get("#join-status").attributes("hidden")).toBeUndefined();
    expect(wrapper.get("#join-status").text()).toContain("Invite sent");
  });

  it("refused: only the refusal text, no \"Invite sent\" line", async () => {
    const { wrapper, store } = await sendInvite();
    store.state = lonely(REFUSED);
    await flushPromises();
    expect(wrapper.get("#join-status").attributes("hidden")).toBeDefined();
    expect(wrapper.text()).not.toContain("Invite sent");
    expect(wrapper.get("#join-wait").text()).toBe("This invite cannot be used.");
  });
});
