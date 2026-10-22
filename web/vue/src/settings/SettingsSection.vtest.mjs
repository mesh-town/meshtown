import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { CONFIRM_STOP_TEXT, STOPPED_NOTE } from "../common/pac-texts.ts";
import { NET_NAME } from "../common/words.ts";
import { ApiError, api } from "../core/api.ts";
import { baseConfig, baseState, mountSection } from "../testing/section.mjs";
import SettingsSection from "./SettingsSection.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const SOCKS = '[aria-label="Proxy port (SOCKS5 and HTTP)"]';
const META = { read_only: [], restart_fields: [], restart_pending: [], cli_overrides: [] };
let mounted;

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  mounted.wrapper.unmount();
  vi.useRealTimers();
});

function mountSettings() {
  const config = baseConfig({ socks: { enabled: true, port: 1080 } });
  return (mounted = mountSection(SettingsSection, { config, configMeta: META, configFile: { writable: true } }));
}

describe("SettingsSection", () => {
  it("with no edits the save bar is hidden; after a field edit it is visible and \"Save\" is enabled", async () => {
    const { wrapper } = mountSettings();
    expect(wrapper.get("#settings-save-bar").attributes("hidden")).toBeDefined();
    await wrapper.get(SOCKS).setValue("1081");
    expect(wrapper.get("#settings-save-bar").attributes("hidden")).toBeUndefined();
    expect(wrapper.get("#settings-save-btn").attributes("disabled")).toBeUndefined();
    expect(wrapper.get("#settings-dirty-hint").attributes("hidden")).toBeUndefined();
  });

  it("\"Save\" — PUT carries the field edit", async () => {
    api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
    const { wrapper, store } = mountSettings();
    await wrapper.get(SOCKS).setValue("1081");
    await wrapper.get("#settings-save-btn").trigger("click");
    await flushPromises();
    const [method, path, body] = api.mock.calls[0];
    expect([method, path]).toEqual(["PUT", "/api/config"]);
    expect(body.config.socks.port).toBe(1081);
    expect(store.configRev).toBe(2);
  });

  it("a 409 response — conflict card with two buttons, no retry", async () => {
    api.mockRejectedValue(new ApiError(409, "stale_rev", "stale", { rev: 5 }));
    const { wrapper } = mountSettings();
    await wrapper.get(SOCKS).setValue("1081");
    await wrapper.get("#settings-save-btn").trigger("click");
    await flushPromises();
    await vi.advanceTimersByTimeAsync(5000);
    expect(api).toHaveBeenCalledTimes(1);
    const slot = wrapper.get("#settings-conflict-slot");
    expect(slot.text()).toContain("The settings changed while you were editing");
    expect(slot.findAll("button").map((b) => b.text())).toEqual([
      "Reload (your changes will be lost)", "Save my changes over them",
    ]);
  });

  describe("\"unsaved\" label", () => {
    it("a field edit sets store.settingsDirty, cancel clears it", async () => {
      const { wrapper, store } = mountSettings();
      expect(store.settingsDirty).toBe(false);
      await wrapper.get(SOCKS).setValue("1081");
      expect(store.settingsDirty).toBe(true);
      await wrapper.get("#settings-reset-btn").trigger("click");
      expect(store.settingsDirty).toBe(false);
    });

    it("unmounting the section clears the label", async () => {
      const { wrapper, store } = mountSettings();
      await wrapper.get(SOCKS).setValue("1081");
      wrapper.unmount();
      expect(store.settingsDirty).toBe(false);
      mounted = { wrapper: { unmount() {} } };
    });
  });

  it("the section starts with a hint about \"after restart\" fields", () => {
    const { wrapper } = mountSettings();
    expect(wrapper.text()).toContain("Fields marked \"after restart\" take effect after the node restarts");
    expect(wrapper.text()).toContain("a \"Restart Mesh Town\" button appears at the bottom");
  });

  it("conflict — no revision, text about another tab; \"Advanced\"", async () => {
    api.mockRejectedValue(new ApiError(409, "stale_rev", "stale", { rev: 5 }));
    const { wrapper } = mountSettings();
    await wrapper.get(SOCKS).setValue("1081");
    await wrapper.get("#settings-save-btn").trigger("click");
    await flushPromises();
    const text = wrapper.get("#settings-conflict-slot").text();
    expect(text).toContain("They were changed in another tab or by Mesh Town itself. Choose which changes to keep.");
    expect(text).not.toContain("revision");
    expect(wrapper.get("#settings-expert-details summary").text()).toBe("Advanced");
  });

  it("file banner: title, file replacement and field errors via the fieldErrorLine string", () => {
    const config = baseConfig({ socks: { enabled: true, port: 1080 } });
    const configFile = {
      writable: true, state: "invalid", message: "",
      field_errors: [{ path: "socks.port", message: "not a number" }, { path: "node.name", message: "empty" }],
    };
    mounted = mountSection(SettingsSection, { config, configMeta: META, configFile });
    const banner = mounted.wrapper.get("#settings-file-banner").text();
    expect(banner).toContain("The settings file can't be read");
    expect(banner).toContain("The settings file is damaged");
    expect(banner).toContain("the old one will stay next to it under the name");
    expect(banner).toContain("defaults are used for them");
    expect(banner).toContain("Proxy port (SOCKS5 and HTTP) (socks.port): not a number");
    expect(banner).toContain("node.name: empty");
  });

  describe("joining another network", () => {
    it("with a network in the slot, the \"Switch to another network\" region", () => {
      const { wrapper } = mountSettings();
      const slot = wrapper.get("#net-join-slot");
      expect(slot.find("#join-card").exists()).toBe(true);
      expect(slot.get("#join-card-title").text()).toBe("Switch to another network");
    });

    it("with no network there is no join card in \"Settings\": it is on \"Home\"", () => {
      const config = baseConfig({ socks: { enabled: true, port: 1080 } });
      mounted = mountSection(SettingsSection, {
        state: baseState({ network: null, peers: [] }), config, configMeta: META, configFile: { writable: true },
      });
      expect(mounted.wrapper.find("#join-card").exists()).toBe(false);
    });
  });

  it("the stop button and hint name the program", () => {
    const { wrapper } = mountSettings();
    const btn = wrapper.get("#stop-node-btn");
    expect(btn.text()).toBe("Stop " + NET_NAME);
    const hint = btn.element.nextElementSibling.textContent;
    expect(hint).toContain("\"Start " + NET_NAME + "\"");
    expect(hint).not.toContain("node");
  });

  it("the paragraph under the stop button is the full STOPPED_NOTE: what happens to the browser while the program is stopped", () => {
    const { wrapper } = mountSettings();
    const note = wrapper.get("#stop-node-btn").element.nextElementSibling;
    expect(note.textContent.trim()).toBe(STOPPED_NOTE);
  });

  it("\"Stop\" asks for confirmation with the CONFIRM_STOP_TEXT text; on refusal there is no request", async () => {
    const ask = vi.fn(() => false);
    vi.stubGlobal("confirm", ask);
    const { wrapper } = mountSettings();
    await wrapper.get("#stop-node-btn").trigger("click");
    await flushPromises();
    expect(ask).toHaveBeenCalledWith(CONFIRM_STOP_TEXT);
    expect(api).not.toHaveBeenCalled();
  });
});
