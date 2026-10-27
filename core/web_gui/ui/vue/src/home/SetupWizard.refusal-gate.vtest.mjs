/* The join screen hides a refusal the user has seen once a new attempt starts, and shows it again if it comes back. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { baseState } from "../testing/section.mjs";
import { mountWizard, step, unmountWizard } from "../testing/wizard.mjs";
import { INSPECT_DELAY_MS } from "./inspect-preview.ts";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const lonely = (invite_join) => baseState({ network: null, peers: [], node: { ...baseState().node, invite_join } });
const WAITING = { state: "waiting", message: "waiting", label: "family", expires_unix: 1 };
const REFUSED = { ...WAITING, state: "refused", message: "This invite cannot be used." };
let mounted;

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  api.mockReset();
  location.hash = "";
});
afterEach(() => {
  unmountWizard(mounted);
  vi.useRealTimers();
});

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

  it("a refusal keeps the wizard on the join screen", async () => {
    const { wrapper } = await openForm(REFUSED);
    expect(step(wrapper)).toBe("join");
  });

  it("a successful join after a refusal leaves no refusal and the wizard moves on", async () => {
    const { wrapper, store } = await openForm(REFUSED);
    await pasteAndJoin(wrapper);
    store.state = baseState();
    await flushPromises();
    expect(wrapper.find("#join-refused").exists()).toBe(false);
    expect(wrapper.find("#home-start-choice").exists()).toBe(false);
    expect(step(wrapper)).not.toBe("join");
  });
});

async function openForm(info) {
  api.mockImplementation(async (method, path) => {
    if (path === "/api/network/inspect") return { label: "family", id_hex: "aa" };
    return { label: "family", restart: false, waiting: true, slot: "main" };
  });
  mounted = mountWizard({ hasNet: false, state: lonely(info) });
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
