import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { baseConfig, baseState, mountSection } from "../testing/section.mjs";
import { PEER_FULL } from "../testing/section.mjs";
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

describe("PortsSection", () => {
  it("\"Unpublish\" — PUT without this port, \"Saved\"", async () => {
    api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
    const { wrapper } = mountPorts();
    await wrapper.get('[data-port-unpublish="8080"]').trigger("click");
    await flushPromises();
    const [method, path, body] = api.mock.calls[0];
    expect([method, path]).toEqual(["PUT", "/api/config"]);
    expect(body.config.ports.published.map((p) => p.port)).toEqual([9090]);
    expect(wrapper.get("#ports-status").text()).toBe("Saved");
  });

  it("a double click while the api hangs — the buttons are disabled, one call", async () => {
    let release;
    api.mockImplementation((_m, _p, body) => new Promise((resolve) => { release = () => resolve({ config: body.config, rev: 2 }); }));
    const { wrapper } = mountPorts();
    const btn = wrapper.get('[data-port-unpublish="8080"]');
    await btn.trigger("click");
    expect(btn.attributes("disabled")).toBeDefined();
    expect(wrapper.get("#port-add-btn").attributes("disabled")).toBeDefined();
    await btn.trigger("click");
    expect(api).toHaveBeenCalledTimes(1);
    release();
    await flushPromises();
    expect(btn.attributes("disabled")).toBeUndefined();
  });

  describe("who the port is open for", () => {
    it("the \"Shared with\" column: \"all members\" for for: all and the node name for for: [id]", () => {
      mounted = mountSection(PortsSection, {
        props: { slot: "main" },
        state: baseState({ ports: { published: OWN, network: [] } }),
        config: baseConfig({ ports: { published: [{ port: 8080, name: "web", for: "all" }, { port: 9090, name: "db", for: [PEER_FULL] }], opened: [] } }),
      });
      const rows = mounted.wrapper.findAll("#ports-own-tbody tr");
      expect(rows[0].text()).toContain("all members");
      expect(rows[1].text()).toContain("peer-one");
    });

    it("publishing to a chosen node: PUT with for: [id_full], an empty choice — the text in #port-add-error", async () => {
      api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
      const { wrapper } = mountPorts();
      await wrapper.get("#port-add-port").setValue("8081");
      await wrapper.get("#port-add-audience-nodes").setValue();
      await wrapper.get("#port-add-btn").trigger("click");
      await flushPromises();
      expect(wrapper.get("#port-add-error").text()).toBe("Select at least one member.");
      expect(api).not.toHaveBeenCalled();
      await wrapper.get('[data-port-audience-node="' + PEER_FULL + '"]').setValue(true);
      await wrapper.get("#port-add-btn").trigger("click");
      await flushPromises();
      const added = api.mock.calls.at(-1)[2].config.ports.published.find((p) => p.port === 8081);
      expect(added.for).toEqual([PEER_FULL]);
    });
  });

  describe("network ports", () => {
    const row = (opened) => ({ node: "ab", node_name: "finch", port: 80, name: "luci", published: true, online: true, opened });
    const mountNet = (opened, openedCfg = []) => (mounted = mountSection(PortsSection, {
      props: { slot: "main" },
      state: baseState({ ports: { published: [], network: [row(opened)] } }),
      config: baseConfig({ ports: { published: [], opened: openedCfg } }),
    }));

    it("the number draft starts with the port plus 2000 and \"Open\" writes it to the config", async () => {
      api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
      const { wrapper } = mountNet(null);
      expect(wrapper.get('[data-port-local="ab/80"]').element.value).toBe("2080");
      await wrapper.get('[data-port-open="ab/80"]').trigger("click");
      await flushPromises();
      expect(api.mock.calls.at(-1)[2].config.ports.opened).toEqual([{ node: "ab", port: 80, local: 2080 }]);
    });

    it("bind_failed: no \"opened\", there is an error, a field, \"Open\" and \"Close\"; opening again changes local", async () => {
      api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
      const failed = { local: 80, state: "bind_failed", error: "127.0.0.1:80 cannot be bound: Permission denied (os error 13)" };
      const { wrapper } = mountNet(failed, [{ node: "ab", port: 80, local: 80 }]);
      const text = wrapper.get('[data-port-row="ab/80"]').text();
      expect(text).not.toContain("127.0.0.1:80 open");
      expect(text).toContain("port 80 is busy — choose another number");
      expect(text).toContain("Couldn't open: ");
      expect(wrapper.find('[data-port-close="ab/80"]').exists()).toBe(true);
      await wrapper.get('[data-port-local="ab/80"]').setValue("8080");
      await wrapper.get('[data-port-open="ab/80"]').trigger("click");
      await flushPromises();
      expect(api.mock.calls.at(-1)[2].config.ports.opened).toEqual([{ node: "ab", port: 80, local: 8080 }]);
    });

    it("listening: the address and the \"open\" badge as before", () => {
      const { wrapper } = mountNet({ local: 2080, state: "listening", error: null });
      const text = wrapper.get('[data-port-row="ab/80"]').text();
      expect(text).toContain("127.0.0.1:2080CopyopenClose");
    });
  });
});
