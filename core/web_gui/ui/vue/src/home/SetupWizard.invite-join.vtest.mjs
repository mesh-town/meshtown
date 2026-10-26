/* The setup wizard with the one invite form: an invite that waits for a member shows the progress block; when the network is in, the wizard hands over. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { baseState, mountSection } from "../testing/section.mjs";
import { INSPECT_DELAY_MS } from "./inspect-preview.ts";
import SetupWizard from "./SetupWizard.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

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

describe("SetupWizard > an invite that waits", () => {
  it("waiting: the progress block, then the network is in and the wizard hands over", async () => {
    api.mockImplementation(async (method, path) => {
      if (path === "/api/network/inspect") return { label: "family", id_hex: "aa" };
      return { label: "family", restart: false, waiting: true, slot: "main" };
    });
    mounted = mountSection(SetupWizard, { state: baseState({ network: null, peers: [] }) });
    await mounted.wrapper.get("#home-start-join-btn").trigger("click");
    await mounted.wrapper.get("#join-key-input").setValue("mesh-invite2-abc");
    await vi.advanceTimersByTimeAsync(INSPECT_DELAY_MS);
    await flushPromises();
    await mounted.wrapper.get("#join-btn").trigger("click");
    await flushPromises();
    expect(mounted.wrapper.find("#home-start-choice").exists()).toBe(true);
    expect(mounted.wrapper.find(".join-progress, #join-wait, #join-status").exists(), "the wait is shown").toBe(true);
    mounted.store.state = baseState();
    await flushPromises();
    expect(mounted.wrapper.find("#home-start-choice").exists()).toBe(false);
  });
});
