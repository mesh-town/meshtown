/* The setup wizard with the one invite form: an invite that waits for a member shows the progress block;
 * when the network is in, the wizard moves on to the next question and the invite card appears, quiet, under the setup. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { baseState } from "../testing/section.mjs";
import { PAC_URL, cursor, mountWizard, step, unmountWizard } from "../testing/wizard.mjs";
import { INSPECT_DELAY_MS } from "./inspect-preview.ts";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

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

describe("SetupWizard > an invite that waits", () => {
  it("waiting: the progress block stays on the join screen", async () => {
    const { wrapper } = await joinWaiting();
    expect(step(wrapper)).toBe("join");
    expect(wrapper.find("#home-start-choice").exists()).toBe(true);
    expect(wrapper.find(".join-progress, #join-wait, #join-status").exists(), "the wait is shown").toBe(true);
  });

  it("the network comes in: the wizard moves to the scope question", async () => {
    const { wrapper, store } = await joinWaiting();
    store.state = joinedState();
    await flushPromises();
    expect(wrapper.find("#home-start-choice").exists()).toBe(false);
    expect(cursor(wrapper)).toBe("scope");
  });

  it("the invite card is not offered while there is no network", async () => {
    const { wrapper } = await joinWaiting();
    expect(wrapper.find("#netkey-invite-btn").exists()).toBe(false);
  });

  it("after the join the invite card is there but quiet, outside the setup region", async () => {
    const { wrapper, store } = await joinWaiting();
    store.state = joinedState();
    await flushPromises();
    const invite = wrapper.get("#netkey-invite-btn");
    expect(invite.classes(), "quiet under the wizard").not.toContain("btn-primary");
    const region = wrapper.get('[role="region"][aria-labelledby="home-wizard-title"]');
    expect(region.find("#netkey-invite-btn").exists()).toBe(false);
  });
});

function joinedState() {
  return baseState({ socks: { enabled: true, listen: "127.0.0.1:1080", pac_url: PAC_URL } });
}

async function joinWaiting() {
  api.mockImplementation(async (method, path) => {
    if (path === "/api/network/inspect") return { label: "family", id_hex: "aa" };
    return { label: "family", restart: false, waiting: true, slot: "main" };
  });
  mounted = mountWizard({ hasNet: false });
  const { wrapper } = mounted;
  await wrapper.get("#home-start-join-btn").trigger("click");
  await wrapper.get("#join-key-input").setValue("mesh-invite2-abc");
  await vi.advanceTimersByTimeAsync(INSPECT_DELAY_MS);
  await flushPromises();
  await wrapper.get("#join-btn").trigger("click");
  await flushPromises();
  return mounted;
}
