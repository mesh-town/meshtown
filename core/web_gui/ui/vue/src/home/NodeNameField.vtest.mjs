/* NodeNameField: the name field with a configurable id base. */

import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { baseState, mountSection } from "../testing/section.mjs";
import NodeNameField from "./NodeNameField.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

let mounted;
afterEach(() => mounted.wrapper.unmount());

const mountField = (props) => (mounted = mountSection(NodeNameField, { state: baseState(), props }));

describe("NodeNameField", () => {
  it("without the prop — the ids of the field, status and note are based on \"field-node-name\"", () => {
    const { wrapper } = mountField();
    expect(wrapper.get("#field-node-name").element.value).toBe("finch-unwary");
    expect(wrapper.get('label[for="field-node-name"]').text()).toBe("Name of this computer");
    expect(wrapper.find("#field-node-name-status").exists()).toBe(true);
    expect(wrapper.find("#field-node-name-cli-note").exists()).toBe(true);
  });

  it("with its own idBase — all three ids are based on it", () => {
    const { wrapper } = mountField({ idBase: "settings-node-name" });
    expect(wrapper.find("#field-node-name").exists()).toBe(false);
    expect(wrapper.get('label[for="settings-node-name"]').exists()).toBe(true);
    expect(wrapper.find("#settings-node-name-status").exists()).toBe(true);
    expect(wrapper.find("#settings-node-name-cli-note").exists()).toBe(true);
  });

  it("Enter saves the name", async () => {
    api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
    const { wrapper } = mountField({ idBase: "settings-node-name" });
    await wrapper.get("#settings-node-name").setValue("office");
    await wrapper.get("#settings-node-name").trigger("keydown", { key: "Enter" });
    await flushPromises();
    expect(api.mock.calls[0][2].config.node.name).toBe("office");
    expect(wrapper.get("#settings-node-name-status").text()).toBe("Saved");
  });
});

describe("NodeNameField: withSave", () => {
  const saveButton = (wrapper) => wrapper.findAll("button").find((b) => b.text() === "Save");

  it("without the prop there is no button", () => {
    const { wrapper } = mountField();
    expect(wrapper.findAll("button")).toHaveLength(0);
  });

  it("with the prop a \"Save\" button follows the field", () => {
    const { wrapper } = mountField({ withSave: true });
    const save = saveButton(wrapper);
    expect(save, "a Save button").toBeDefined();
    expect(save.attributes("type")).toBe("button");
    const follows = wrapper.get("#field-node-name").element.compareDocumentPosition(save.element) & Node.DOCUMENT_POSITION_FOLLOWING;
    expect(follows, "the button comes after the field").toBeTruthy();
  });

  it("a click on \"Save\" saves the name and shows \"Saved\"", async () => {
    api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
    const { wrapper } = mountField({ withSave: true });
    await wrapper.get("#field-node-name").setValue("office");
    await saveButton(wrapper).trigger("click");
    await flushPromises();
    expect(api.mock.calls[0][2].config.node.name).toBe("office");
    expect(wrapper.get("#field-node-name-status").text()).toBe("Saved");
    expect(wrapper.get("#field-node-name-status").classes()).toContain("saved");
  });

  it("Enter still saves the name in this mode", async () => {
    api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
    const { wrapper } = mountField({ withSave: true });
    await wrapper.get("#field-node-name").setValue("office");
    await wrapper.get("#field-node-name").trigger("keydown", { key: "Enter" });
    await flushPromises();
    expect(api.mock.calls[0][2].config.node.name).toBe("office");
  });

  it("the name is pinned by the command line — the button is disabled", () => {
    const { wrapper } = mountSection(NodeNameField, {
      state: baseState(),
      props: { withSave: true },
      configMeta: { read_only: ["node.name"], restart_fields: [], restart_pending: [], cli_overrides: [] },
    });
    mounted = { wrapper };
    expect(saveButton(wrapper).attributes("disabled")).toBeDefined();
  });
});

