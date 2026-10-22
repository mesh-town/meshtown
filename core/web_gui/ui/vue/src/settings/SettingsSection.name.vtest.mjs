/* "Settings" → "Main": the node name field saves itself, bypassing the draft. */

import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { STORE_KEY } from "../core/store.ts";
import ThisComputerCard from "../home/ThisComputerCard.vue";
import { baseConfig, mountSection } from "../testing/section.mjs";
import SettingsSection from "./SettingsSection.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const META = { read_only: [], restart_fields: [], restart_pending: [], cli_overrides: [] };
const NAME = "#settings-main-card #settings-node-name";
let mounted;
let home;

afterEach(() => {
  mounted.wrapper.unmount();
  home?.unmount();
  home = undefined;
});

const mountSettings = () => (mounted = mountSection(SettingsSection, { config: baseConfig(), configMeta: META, configFile: { writable: true } }));

describe("SettingsSection: node name", () => {
  it("the \"Main\" card has a name field, its id does not clash with \"Home\"", () => {
    const { wrapper } = mountSettings();
    expect(wrapper.get(NAME).element.value).toBe("finch-unwary");
    expect(wrapper.find("#field-node-name").exists()).toBe(false);
  });

  it("editing the name does not make the settings draft \"dirty\"", async () => {
    const { wrapper } = mountSettings();
    await wrapper.get(NAME).setValue("office");
    expect(wrapper.get("#settings-save-bar").attributes("hidden")).toBeDefined();
    expect(wrapper.get("#settings-dirty-hint").attributes("hidden")).toBeDefined();
  });

  it("the name saved in \"Settings\" appears in the \"Home\" field", async () => {
    api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
    const { wrapper, store } = mountSettings();
    home = mount(ThisComputerCard, { attachTo: document.body, global: { provide: { [STORE_KEY]: store } } });
    await wrapper.get(NAME).setValue("office");
    await wrapper.get(NAME).trigger("keydown", { key: "Enter" });
    await flushPromises();
    expect(home.get("#field-node-name").element.value).toBe("office");
  });
});
