import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { PEER_FULL, baseConfig, baseState, mountSection } from "../testing/section.mjs";
import PortsSection from "./PortsSection.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const OWN = [{ port: 8080, name: "web", for: "all", local: "ok" }, { port: 9090, name: "db", for: "all", local: "ok" }];
let mounted;
const mountPorts = () => (mounted = mountSection(PortsSection, {
  props: { slot: "main" },
  state: baseState({ ports: { published: OWN, network: [] } }),
  config: baseConfig({ ports: { published: OWN.map(({ port, name, for: f }) => ({ port, name, for: f })), opened: [] } }),
}));

afterEach(() => { mounted.wrapper.unmount(); api.mockReset(); });

describe("PortsSection: editing a published port", () => {
  it("\"Edit\" expands the editor with the current name; only one is open at a time", async () => {
    const { wrapper } = mountPorts();
    await wrapper.get('[data-port-edit="8080"]').trigger("click");
    expect(wrapper.get('[data-port-edit-name="8080"]').element.value).toBe("web");
    await wrapper.get('[data-port-edit="9090"]').trigger("click");
    expect(wrapper.findAll("[data-port-edit-save]")).toHaveLength(1);
    expect(wrapper.find('[data-port-edit-name="8080"]').exists()).toBe(false);
  });

  it("\"Selected nodes\" + a node + \"Save\" — PUT with for: [id], the editor is closed", async () => {
    api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
    const { wrapper } = mountPorts();
    await wrapper.get('[data-port-edit="8080"]').trigger("click");
    await wrapper.get("#port-edit-8080-audience-nodes").setValue();
    await wrapper.get('#port-edit-8080-audience-list [data-port-audience-node="' + PEER_FULL + '"]').setValue(true);
    await wrapper.get("[data-port-edit-save]").trigger("click");
    await flushPromises();
    const saved = api.mock.calls.at(-1)[2].config.ports.published.find((p) => p.port === 8080);
    expect(saved).toEqual({ port: 8080, name: "web", for: [PEER_FULL] });
    expect(wrapper.find("[data-port-edit-save]").exists()).toBe(false);
  });

  it("an empty node choice — an error under the editor, no request, the editor stays open", async () => {
    const { wrapper } = mountPorts();
    await wrapper.get('[data-port-edit="8080"]').trigger("click");
    await wrapper.get("#port-edit-8080-audience-nodes").setValue();
    await wrapper.get("[data-port-edit-save]").trigger("click");
    await flushPromises();
    expect(wrapper.get("[data-port-edit-save]").exists()).toBe(true);
    expect(wrapper.text()).toContain("Select at least one member.");
    expect(api).not.toHaveBeenCalled();
  });

  it("\"Cancel\" closes the editor without a request", async () => {
    const { wrapper } = mountPorts();
    await wrapper.get('[data-port-edit="8080"]').trigger("click");
    await wrapper.get("[data-port-edit-cancel]").trigger("click");
    expect(wrapper.find("[data-port-edit-save]").exists()).toBe(false);
    expect(api).not.toHaveBeenCalled();
  });
});
