/* The setup wizard hides a refusal the user has seen once a new attempt starts, and shows it again if it comes back. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { baseState, mountSection } from "../testing/section.mjs";
import { INSPECT_DELAY_MS } from "./inspect-preview.ts";
import SetupWizard from "./SetupWizard.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const lonely = (invite_join) => baseState({ network: null, peers: [], node: { ...baseState().node, invite_join } });
const WAITING = { state: "waiting", message: "waiting", label: "family", expires_unix: 1 };
const REFUSED = { ...WAITING, state: "refused", message: "This invite cannot be used." };
let mounted;

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  api.mockReset();
  localStorage.clear();
  location.hash = "";
});
afterEach(() => {
  mounted.wrapper.unmount();
  vi.useRealTimers();
});

async function openForm(info) {
  api.mockImplementation(async (method, path) => {
    if (path === "/api/network/inspect") return { label: "family", kind: "Peer", id_hex: "aa" };
    return { label: "family", restart: false, waiting: true, slot: "main" };
  });
  mounted = mountSection(SetupWizard, { state: lonely(info) });
  await mounted.wrapper.get("#home-start-join-btn").trigger("click");
  return mounted;
}

async function pasteAndJoin(wrapper) {
  await wrapper.get("#join-key-input").setValue("mesh-invite2-abc");
  await vi.advanceTimersByTimeAsync(INSPECT_DELAY_MS);
  await flushPromises();
  await wrapper.get("#join-btn").trigger("click");
  await flushPromises();
}

describe("SetupWizard > refusal gate", () => {
  it("a refusal without a new attempt is shown with its lead", async () => {
    const { wrapper } = await openForm(REFUSED);
    expect(wrapper.get("#join-refused").text()).toBe("Couldn't join \"family\". This invite cannot be used.");
  });

  it("a new attempt hides the old refusal until the snapshot changes", async () => {
    const { wrapper } = await openForm(REFUSED);
    await pasteAndJoin(wrapper);
    expect(wrapper.find("#join-refused").exists()).toBe(false);
  });

  it("the same refusal coming back after waiting is shown again", async () => {
    const { wrapper, store } = await openForm(REFUSED);
    await pasteAndJoin(wrapper);
    store.state = lonely(WAITING);
    await flushPromises();
    store.state = lonely(REFUSED);
    await flushPromises();
    expect(wrapper.get("#join-refused").text()).toContain("This invite cannot be used.");
  });

  it("an empty field and Join is no attempt: the refusal stays", async () => {
    const { wrapper } = await openForm(REFUSED);
    await wrapper.get("#join-btn").trigger("click");
    await flushPromises();
    expect(wrapper.get("#join-refused").text()).toContain("This invite cannot be used.");
  });

  it("a different refusal after a new attempt is shown", async () => {
    const { wrapper, store } = await openForm(REFUSED);
    await pasteAndJoin(wrapper);
    store.state = lonely({ ...REFUSED, message: "The network is full." });
    await flushPromises();
    expect(wrapper.get("#join-refused").text()).toContain("The network is full.");
  });

  it("a successful join after a refusal leaves no refusal and the wizard hands over", async () => {
    const { wrapper, store } = await openForm(REFUSED);
    await pasteAndJoin(wrapper);
    store.state = baseState();
    await flushPromises();
    expect(wrapper.find("#join-refused").exists()).toBe(false);
    expect(wrapper.find("#home-start-choice").exists()).toBe(false);
  });
});
