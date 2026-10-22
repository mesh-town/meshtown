/* The page skeleton: tabs and panels, readiness for e2e, strips and the link to polling. The network is mocked
 * (`vi.mock` of the `api` core); the shell's polling goes through the same mock `api`. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { ApiError, api } from "../core/api.ts";
import { baseConfig, baseState, mountSection } from "../testing/section.mjs";
import AppShell from "./AppShell.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const NAMES = ["Home", "Members", "Site checks", "Ports", "Diagnostics", "Log", "Settings"];
let mounted;

beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal("fetch", async () => new Response("{}", { status: 404 })); // sign-in status: "old node", the shell as is
  sessionStorage.clear();
  location.hash = "";
  answer({ state: () => baseState({ config_rev: 1 }) });
});
afterEach(() => {
  mounted.wrapper.unmount();
  vi.useRealTimers();
  location.hash = "";
});

describe("AppShell", () => {
  describe("tabs and panels", () => {
    it("seven role=tab tabs and seven tabpanels named after the tabs", async () => {
      const { wrapper } = await mountShell();
      expect(wrapper.findAll("#tabs [role=tab]").map((t) => t.text())).toEqual(NAMES);
      const panels = wrapper.findAll("[role=tabpanel][id^=sec-]");
      expect(panels).toHaveLength(7);
      const labelled = panels.map((p) => wrapper.get("#" + p.attributes("aria-labelledby")).text());
      expect(labelled).toEqual(NAMES);
    });

    it("the active one by the store — aria-selected, the other panels hidden", async () => {
      const { wrapper, store } = await mountShell();
      store.activeSection = "log";
      await flushPromises();
      expect(wrapper.get("#tab-log").attributes("aria-selected")).toBe("true");
      expect(wrapper.get("#tab-home").attributes("aria-selected")).toBe("false");
      const visible = wrapper.findAll("[role=tabpanel][id^=sec-]").filter((p) => p.attributes("hidden") === undefined);
      expect(visible.map((p) => p.attributes("id"))).toEqual(["sec-log"]);
    });

    it("the starting section — from the address hash", async () => {
      location.hash = "#ports";
      const { wrapper, store } = await mountShell();
      expect(store.activeSection).toBe("ports");
      expect(wrapper.get("#sec-ports").attributes("hidden")).toBeUndefined();
    });

    it("\"Settings\" starts with a hint about \"after restart\" fields", async () => {
      const { wrapper } = await mountShell();
      expect(wrapper.get("#sec-settings").text()).toContain("Fields marked \"after restart\"");
    });

    it("every section has a heading except \"Home\"", async () => {
      const { wrapper } = await mountShell();
      const titles = wrapper.findAll(".tab-title").map((t) => t.text());
      expect(titles).toEqual(NAMES.slice(1));
    });
  });

  describe("readiness for e2e", () => {
    it("there is no snapshot yet — data-ready is absent", async () => {
      api.mockImplementation(() => new Promise(() => {}));
      const { wrapper } = await mountShell({ state: null, configRev: null });
      expect(wrapper.get(".shell").attributes("data-ready")).toBeUndefined();
    });

    it("the snapshot and config revision exist — data-ready=true", async () => {
      const { wrapper } = await mountShell();
      expect(wrapper.get(".shell").attributes("data-ready")).toBe("true");
    });

    it("the snapshot came from polling, the config loaded later — data-ready appears", async () => {
      answer({ state: () => baseState({ config_rev: 1 }), config: () => ({ config: baseConfig(), rev: 1 }) });
      const { wrapper } = await mountShell({ state: null, configRev: null });
      await vi.advanceTimersByTimeAsync(0);
      await flushPromises();
      expect(wrapper.get(".shell").attributes("data-ready")).toBe("true");
    });
  });

  describe("\"unsaved\" label", () => {
    it("store.settingsDirty sets has-dirty on the \"Settings\" tab and keeps it when the tab changes", async () => {
      const { wrapper, store } = await mountShell();
      store.settingsDirty = true;
      store.activeSection = "peers";
      await flushPromises();
      expect(wrapper.get("#tab-settings").classes()).toContain("has-dirty");
    });
  });

  describe("node does not respond to the token (401)", () => {
    it("the sign-in screen instead of the shell (details — login/AppShell.login.vtest.mjs); the \"old link\" strip is hidden", async () => {
      answer({ state: () => { throw new ApiError(401, "bad_token"); } });
      const { wrapper } = await mountShell({ state: null, configRev: null });
      expect(wrapper.find("#login-form").exists()).toBe(true);
      expect(wrapper.find("#token-url-input").exists()).toBe(false);
      expect(wrapper.get("#conn-strip").attributes("hidden")).toBeDefined();
    });
  });

  describe("settings changed in another tab", () => {
    it("\"Reload\" calls GET /api/config and clears the strip", async () => {
      answer({ state: () => baseState({ config_rev: 2 }), config: () => ({ config: baseConfig(), rev: 2 }) });
      const { wrapper } = await mountShell({ configRev: 1 });
      await flushPromises();
      expect(wrapper.get("#conn-strip").text()).toContain("Settings were changed in another tab or by Mesh Town itself");
      api.mockClear();
      await wrapper.get("#conn-strip-reload").trigger("click");
      await flushPromises();
      expect(api).toHaveBeenCalledWith("GET", "/api/config");
      expect(wrapper.get("#conn-strip").attributes("hidden")).toBeDefined();
    });
  });

  describe("connection to the node lost", () => {
    it("two failed polls — \"Mesh Town is not responding\" on the strip and in the status line", async () => {
      answer({ state: () => { throw new ApiError(0, "network"); } });
      const { wrapper } = await mountShell();
      await vi.advanceTimersByTimeAsync(20000);
      await flushPromises();
      expect(wrapper.get("#conn-strip").text()).toBe("Mesh Town is not responding");
      expect(wrapper.get("#status-line").text()).toBe("Mesh Town is not responding");
    });
  });

  describe("identity strip", () => {
    it("a temporary identity — the strip is shown, otherwise hidden", async () => {
      const { wrapper } = await mountShell();
      expect(wrapper.get("#identity-strip").attributes("hidden")).toBeDefined();
      const node = { ...baseState().node, identity: "temporary", identity_reason: "declined" };
      answer({ state: () => baseState({ config_rev: 1, node }) });
      await vi.advanceTimersByTimeAsync(2000);
      await flushPromises();
      expect(wrapper.get("#identity-strip").attributes("hidden")).toBeUndefined();
      expect(wrapper.get("#identity-strip").classes()).toContain("identity-strip-declined");
    });
  });
});

// Responses of the mock `api` by path; an undescribed request — `null`.
function answer({ state, config } = {}) {
  api.mockImplementation(async (_method, path) => {
    if (path === "/api/state") return state ? state() : null;
    if (path === "/api/config") return config ? config() : { config: baseConfig(), rev: 1 };
    return null;
  });
}

async function mountShell(detail = {}) {
  mounted = mountSection(AppShell, { configMeta: { read_only: [], restart_fields: [], restart_pending: [], cli_overrides: [] }, ...detail });
  await flushPromises();
  return mounted;
}
