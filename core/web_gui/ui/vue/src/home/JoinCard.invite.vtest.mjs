/* The join card with the one invite form: the place the node picks, the note, the move to another network's tab, the restart promise only for `main`. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { ApiError, api } from "../core/api.ts";
import { NOTE_NEW_AT_LIMIT } from "./invite-join-texts.ts";
import { baseState, mountSection } from "../testing/section.mjs";
import { JOIN_PROMISE_MS, JOIN_STALLED } from "./join-actions.ts";
import { INSPECT_DELAY_MS } from "./inspect-preview.ts";
import { leadsToArrival, resetNetBanner, useNetBanner } from "./add-net-banner.ts";
import JoinCard from "./JoinCard.vue";
import SetupWizard from "./SetupWizard.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const INVITE = "mesh-invite2-abc";
let mounted;

const route = (table) => api.mockImplementation(async (method, path) => {
  if (table[path] === undefined) throw new Error("unexpected request " + path);
  return table[path];
});
const brief = (id, label, over = {}) => ({ id, label, id_hex: id + "-hex", status: "running", joining: false, me_excluded: false, ...over });
async function send(wrapper) {
  await wrapper.get("#join-key-input").setValue(INVITE);
  await vi.advanceTimersByTimeAsync(INSPECT_DELAY_MS);
  await flushPromises();
}
const press = async (wrapper) => {
  await wrapper.get("#join-btn").trigger("click");
  await flushPromises();
};

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  api.mockReset();
  localStorage.clear();
  history.replaceState(null, "", "/");
  location.hash = "";
  resetNetBanner();
});
afterEach(() => {
  mounted.wrapper.unmount();
  vi.useRealTimers();
});

describe("JoinCard > the one invite form", () => {
  it("`main` was removed: the note says the invite brings the computer back, the join enters `main`", async () => {
    route({
      "/api/network/inspect": { label: "family", id_hex: "main-hex", place: "return", slot: "main" },
      "/api/network/join": { label: "family", restart: true, slot: "main" },
    });
    mounted = mountSection(JoinCard, { state: baseState() });
    mounted.store.nets = [brief("main", "family")];
    await send(mounted.wrapper);
    expect(mounted.wrapper.get("#join-place-note").text()).toBe('This invite brings this computer back to "family".');
    await press(mounted.wrapper);
    expect(mounted.wrapper.get("#join-status").text()).toContain("Done");
    expect(location.hash).toBe("");
  });

  it("`main` is excluded, the invite is for another network: the page goes to the new tab with a banner and promises no restart", async () => {
    route({
      "/api/network/inspect": { label: "other", id_hex: "bb" },
      "/api/network/join": { label: "other", restart: true, slot: "neta22" },
    });
    mounted = mountSection(JoinCard, { state: baseState() });
    mounted.store.nets = [brief("main", "family", { me_excluded: true }), brief("neta22", "other")];
    await send(mounted.wrapper);
    await press(mounted.wrapper);
    expect(location.hash).toBe("#peers/neta22");
    expect(useNetBanner().banner.value.slot).toBe("neta22");
    await vi.advanceTimersByTimeAsync(JOIN_PROMISE_MS + 1);
    await flushPromises();
    expect(mounted.wrapper.text()).not.toContain(JOIN_STALLED);
  });

  it("an invite to another network from Home leads to its tab when it arrives", async () => {
    route({
      "/api/network/inspect": { label: "other", id_hex: "bb" },
      "/api/network/join": { label: null, restart: false, waiting: true, slot: "neta22" },
    });
    mounted = mountSection(JoinCard, { state: baseState() });
    mounted.store.nets = [brief("main", "family", { me_excluded: true })];
    await send(mounted.wrapper);
    await press(mounted.wrapper);
    expect(leadsToArrival("neta22", location.hash, null), "the user is where he sent the form from").toBe(true);
    expect(leadsToArrival("neta22", "#other", null), "the user went elsewhere").toBe(false);
    expect(leadsToArrival("netb33", location.hash, null), "another slot").toBe(false);
  });

  it("at the limit the note goes away when the error appears", async () => {
    api.mockImplementation(async (method, path) => {
      if (path === "/api/network/inspect") return { label: "other", id_hex: "bb", place: "new" };
      throw new ApiError(409, "too_many_nets", "x", { limit: 8 });
    });
    mounted = mountSection(JoinCard, { state: baseState() });
    mounted.store.nets = Array.from({ length: 8 }, (_, i) => brief(i === 0 ? "main" : "net" + i + "x", "n" + i));
    for (const n of mounted.store.nets) mounted.store.slots[n.id] = { state: baseState() };
    await send(mounted.wrapper);
    expect(mounted.wrapper.get("#join-place-note").text()).toBe(NOTE_NEW_AT_LIMIT);
    await press(mounted.wrapper);
    expect(mounted.wrapper.get("#join-key-error").attributes("hidden")).toBeUndefined();
    expect(mounted.wrapper.find("#join-place-note").exists()).toBe(false);
  });

  it("the setup: a Managed invite shows the join progress", async () => {
    route({
      "/api/network/inspect": { label: "firm", id_hex: "cc" },
      "/api/network/join": { label: "firm", restart: true, slot: "main" },
    });
    mounted = mountSection(SetupWizard, { state: baseState({ network: null, peers: [] }) });
    await mounted.wrapper.get("#home-start-join-btn").trigger("click");
    await send(mounted.wrapper);
    await press(mounted.wrapper);
    expect(mounted.wrapper.find(".join-progress").exists()).toBe(true);
  });

  it("a word key in the field of a node with a network: the hint says it replaces the network; an invite has no hint", async () => {
    route({ "/api/network/inspect": { label: "family", id_hex: "aa" } });
    mounted = mountSection(JoinCard, { state: baseState() });
    mounted.store.nets = [brief("main", "family")];
    expect(mounted.wrapper.get("#net-restart-hint").attributes("hidden")).toBeDefined();
    await mounted.wrapper.get("#join-key-input").setValue("mesh2-abc");
    expect(mounted.wrapper.get("#net-restart-hint").text()).toContain("replaces");
    await mounted.wrapper.get("#join-key-input").setValue(INVITE);
    expect(mounted.wrapper.get("#net-restart-hint").attributes("hidden")).toBeDefined();
  });

  describe("the server's key error", () => {
    const OLD_KEY = "This key is from an older version. Ask an admin for a new invite.";
    async function refused() {
      api.mockImplementation(async (method, path) => {
        if (path === "/api/network/join") throw new ApiError(422, "invalid_key", "x", { message: OLD_KEY });
        return {};
      });
      mounted = mountSection(JoinCard, { state: baseState() });
      await mounted.wrapper.get("#join-key-input").setValue("mesh1-oldkey");
      await press(mounted.wrapper);
      return mounted.wrapper;
    }

    it("the error stays until the field changes", async () => {
      const wrapper = await refused();
      expect(wrapper.get("#join-key-error").attributes("hidden")).toBeUndefined();
      expect(wrapper.get("#join-key-error").text()).toBe(OLD_KEY);
      await flushPromises();
      expect(wrapper.get("#join-key-error").text()).toBe(OLD_KEY);
    });

    it("an edit hides the server's key error", async () => {
      const wrapper = await refused();
      await wrapper.get("#join-key-input").setValue("mesh-invite2-abc");
      expect(wrapper.get("#join-key-error").attributes("hidden")).toBeDefined();
    });
  });
});
