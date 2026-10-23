/* "Ports shared with you": the section order and the opened-port cell ("Copy", "Change number"). */

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
let mounted;
let written;

afterEach(() => {
  mounted.wrapper.unmount();
  api.mockReset();
  Reflect.deleteProperty(navigator, "clipboard");
});

function show() {
  written = [];
  Object.defineProperty(navigator, "clipboard", { value: { writeText: async (s) => void written.push(s) }, configurable: true });
  mounted = mountSection(PortsSection, {
    state: baseState({ ports: { published: [], network: [NET_ROW] } }),
    config: baseConfig({ ports: { published: [], opened: [{ node: NODE, port: 80, local: 2080 }] } }),
  });
  return mounted.wrapper;
}

const buttonNamed = (wrapper, text) => wrapper.findAll("#ports-net-tbody button").find((b) => b.text() === text);

describe("the order of the cards", () => {
  it("\"Ports shared with you\" comes before \"My published ports\"", () => {
    const titles = show().findAll("h2").map((h) => h.text());

    expect(titles.indexOf("Ports shared with you")).toBeGreaterThanOrEqual(0);
    expect(titles.indexOf("Ports shared with you")).toBeLessThan(titles.indexOf("My published ports"));
  });
});

describe("an opened port", () => {
  it("shows the address with \"Copy\", the \"open\" badge, \"Close\" and \"Change number\"", () => {
    const wrapper = show();
    const cell = wrapper.findAll("#ports-net-tbody td")[3];

    expect(cell.get(".mono").text()).toBe("127.0.0.1:2080");
    expect(cell.get(".badge").text()).toBe("open");
    expect(cell.get("[aria-label='Copy address 127.0.0.1:2080']").text()).toBe("Copy");
    expect(buttonNamed(wrapper, "Close")).toBeDefined();
    expect(buttonNamed(wrapper, "Change number").classes()).toContain("linkbtn");
  });

  it("\"Copy\" writes 127.0.0.1:<local> and says \"Copied\"", async () => {
    const wrapper = show();
    const copy = wrapper.get("[aria-label='Copy address 127.0.0.1:2080']");

    await copy.trigger("click");
    await flushPromises();

    expect(written).toEqual(["127.0.0.1:2080"]);
    expect(copy.text()).toBe("Copied");
  });

  it("\"Change number\" shows the field with the current number; \"Open on this computer\" sends the new one", async () => {
    api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
    const wrapper = show();

    await buttonNamed(wrapper, "Change number").trigger("click");
    const input = wrapper.get("input[aria-label='port number on this computer']");
    expect(input.element.value).toBe("2080");
    expect(wrapper.find("#ports-net-tbody .badge-ok").exists()).toBe(true);
    await input.setValue("2999");
    await buttonNamed(wrapper, "Open on this computer").trigger("click");
    await flushPromises();

    const [method, path, body] = api.mock.calls[0];
    expect([method, path]).toEqual(["PUT", "/api/config"]);
    expect(body.config.ports.opened.map((o) => o.local)).toEqual([2999]);
    expect(wrapper.find("input[aria-label='port number on this computer']").exists()).toBe(false);
  });
});
