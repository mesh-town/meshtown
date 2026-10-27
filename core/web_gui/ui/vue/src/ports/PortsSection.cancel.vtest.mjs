/* The "Change number" mode of an opened port: it is visible, "Cancel" and Escape leave it without saving. */

import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { baseConfig, baseState, mountSection } from "../testing/section.mjs";
import PortsSection from "./PortsSection.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const NODE = "a".repeat(64);
const NET_ROW = {
  node: NODE, name: "finch", online: true, port: 80, published: true, web_url: "http://finch.meshtown",
  opened: { local: 2080, state: "listening", error: null },
};
const FIELD = "input[aria-label='port number on this computer']";
let mounted;

afterEach(() => {
  mounted.wrapper.unmount();
  api.mockReset();
});

function show() {
  mounted = mountSection(PortsSection, {
    props: { slot: "main" },
    state: baseState({ ports: { published: [], network: [NET_ROW] } }),
    config: baseConfig({ ports: { published: [], opened: [{ node: NODE, port: 80, local: 2080 }] } }),
  });
  return mounted.wrapper;
}

const button = (wrapper, text) => wrapper.findAll("#ports-net-tbody button").find((b) => b.text() === text);
const startChange = async (wrapper) => button(wrapper, "Change number").trigger("click");
const putCalls = () => api.mock.calls.filter(([method]) => method === "PUT");

describe("the mode is visible", () => {
  it("the cell has the `changing` class and the caption; a plain row has neither", async () => {
    const wrapper = show();
    expect(wrapper.find(".opencell.changing").exists()).toBe(false);
    expect(wrapper.text()).not.toContain("New number on this device");
    await startChange(wrapper);
    expect(wrapper.find(".opencell.changing").exists()).toBe(true);
    expect(wrapper.get(".opencell.changing").text()).toContain("New number on this device");
  });
});

describe("Cancel", () => {
  it("leaves the mode without a request; the row is plain again", async () => {
    const wrapper = show();
    await startChange(wrapper);
    await wrapper.get("[data-port-cancel]").trigger("click");
    await flushPromises();
    expect(putCalls()).toHaveLength(0);
    expect(api).not.toHaveBeenCalled();
    expect(wrapper.find(FIELD).exists()).toBe(false);
    expect(wrapper.find(".opencell.changing").exists()).toBe(false);
    expect(button(wrapper, "Change number")).toBeDefined();
  });

  it("is a link button with the text \"Cancel\"", async () => {
    const wrapper = show();
    await startChange(wrapper);
    const cancel = wrapper.get("[data-port-cancel]");
    expect(cancel.text()).toBe("Cancel");
    expect(cancel.classes()).toContain("linkbtn");
  });

  it("opening the mode again starts from the opened number, not from the typed draft", async () => {
    const wrapper = show();
    await startChange(wrapper);
    await wrapper.get(FIELD).setValue("2999");
    await wrapper.get("[data-port-cancel]").trigger("click");
    await startChange(wrapper);
    expect(wrapper.get(FIELD).element.value).toBe("2080");
  });
});

describe("Escape in the field", () => {
  it("leaves the mode without a request", async () => {
    const wrapper = show();
    await startChange(wrapper);
    await wrapper.get(FIELD).setValue("2999");
    await wrapper.get(FIELD).trigger("keydown", { key: "Escape" });
    await flushPromises();
    expect(api).not.toHaveBeenCalled();
    expect(wrapper.find(FIELD).exists()).toBe(false);
    await startChange(wrapper);
    expect(wrapper.get(FIELD).element.value).toBe("2080");
  });
});

describe("Open still saves", () => {
  it("\"Open on this computer\" sends the PUT with the new number", async () => {
    api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
    const wrapper = show();
    await startChange(wrapper);
    await wrapper.get(FIELD).setValue("2999");
    await button(wrapper, "Open on this computer").trigger("click");
    await flushPromises();
    expect(putCalls()).toHaveLength(1);
  });
});
