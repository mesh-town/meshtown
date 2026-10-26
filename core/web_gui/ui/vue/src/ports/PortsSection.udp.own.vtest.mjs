/* "My published ports" for the two protocols: TCP 53 and UDP 53 are two rows, the UDP one has a badge and an unchecked status,
 * and every action (unpublish, edit, publish) works on its own protocol only. */

import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { PEER_FULL, baseConfig, baseState, mountSection } from "../testing/section.mjs";
import PortsSection from "./PortsSection.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const TCP_ROW = { port: 53, name: "dns-tcp", local: "ok" };
const UDP_ROW = { port: 53, proto: "udp", name: "dns-udp", local: "ok" };
const TCP_ENTRY = { port: 53, name: "dns-tcp", for: "all" };
const UDP_ENTRY = { port: 53, proto: "udp", name: "dns-udp", for: "all" };

let mounted;
const mountPorts = () => (mounted = mountSection(PortsSection, {
  props: { slot: "main" },
  state: baseState({ ports: { published: [TCP_ROW, UDP_ROW], network: [] } }),
  config: baseConfig({ ports: { published: [TCP_ENTRY, UDP_ENTRY], opened: [] } }),
}));
const lastPublished = () => api.mock.calls.at(-1)[2].config.ports.published;
const rows = (wrapper) => wrapper.findAll("#ports-own-tbody tr");

afterEach(() => { mounted.wrapper.unmount(); api.mockReset(); });

describe("own ports with the same number in TCP and UDP", () => {
  describe("the rows", () => {
    it("are two: one per protocol", () => {
      const { wrapper } = mountPorts();
      expect(rows(wrapper)).toHaveLength(2);
      expect(rows(wrapper)[0].text()).toContain("dns-tcp");
      expect(rows(wrapper)[1].text()).toContain("dns-udp");
    });

    it("the UDP badge is only on the UDP row", () => {
      const { wrapper } = mountPorts();
      expect(rows(wrapper)[0].find("[data-port-proto]").exists()).toBe(false);
      expect(wrapper.findAll("#ports-own-tbody [data-port-proto]")).toHaveLength(1);
      const badge = rows(wrapper)[1].get('[data-port-proto="udp"]');
      expect(badge.text()).toBe("UDP");
      expect(badge.classes()).toEqual(expect.arrayContaining(["badge", "badge-grey"]));
    });

    it("the port cell is the bare number for TCP and number plus badge for UDP", () => {
      const { wrapper } = mountPorts();
      expect(rows(wrapper)[0].findAll("td")[0].text()).toBe("53");
      expect(rows(wrapper)[1].findAll("td")[0].text()).toMatch(/^53\s*UDP$/);
    });

    it("the UDP status is \"not checked for UDP\" although local is ok; TCP says \"responding\"", () => {
      const { wrapper } = mountPorts();
      expect(rows(wrapper)[0].text()).toContain("responding");
      expect(rows(wrapper)[1].text()).toContain("not checked for UDP");
      expect(rows(wrapper)[1].text()).not.toContain("responding");
    });

    it("the UDP status stays unchecked when local is refused", () => {
      mounted = mountSection(PortsSection, {
        props: { slot: "main" },
        state: baseState({ ports: { published: [{ ...UDP_ROW, local: "refused" }], network: [] } }),
        config: baseConfig({ ports: { published: [UDP_ENTRY], opened: [] } }),
      });
      expect(rows(mounted.wrapper)[0].text()).toContain("not checked for UDP");
      expect(rows(mounted.wrapper)[0].text()).not.toContain("not responding");
    });

    it("the buttons are addressed by the port key: 53 for TCP, 53/udp for UDP", () => {
      const { wrapper } = mountPorts();
      for (const key of ["53", "53/udp"]) {
        expect(wrapper.find('[data-port-edit="' + key + '"]').exists(), "edit " + key).toBe(true);
        expect(wrapper.find('[data-port-unpublish="' + key + '"]').exists(), "unpublish " + key).toBe(true);
      }
    });
  });

  describe("Unpublish", () => {
    it("on the UDP row sends the list without UDP; the TCP entry stays without a proto field", async () => {
      api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
      const { wrapper } = mountPorts();
      await wrapper.get('[data-port-unpublish="53/udp"]').trigger("click");
      await flushPromises();
      expect(api.mock.calls[0].slice(0, 2)).toEqual(["PUT", "/api/config"]);
      expect(lastPublished()).toEqual([TCP_ENTRY]);
      expect("proto" in lastPublished()[0]).toBe(false);
      expect(wrapper.get("#ports-status").text()).toBe("Saved");
    });

    it("on the TCP row sends the list without TCP; the UDP entry keeps proto: udp", async () => {
      api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
      const { wrapper } = mountPorts();
      await wrapper.get('[data-port-unpublish="53"]').trigger("click");
      await flushPromises();
      expect(lastPublished()).toEqual([UDP_ENTRY]);
    });
  });

  describe("Edit on the UDP row", () => {
    it("opens an editor titled \"Edit UDP port 53\" filled with the UDP name", async () => {
      const { wrapper } = mountPorts();
      await wrapper.get('[data-port-edit="53/udp"]').trigger("click");
      expect(wrapper.get(".port-edit h3").text()).toBe("Edit UDP port 53");
      expect(wrapper.get('[data-port-edit-name="53/udp"]').element.value).toBe("dns-udp");
      expect(wrapper.find('[data-port-edit-name="53"]').exists()).toBe(false);
    });

    it("uses the id prefix port-edit-53-udp-", async () => {
      const { wrapper } = mountPorts();
      await wrapper.get('[data-port-edit="53/udp"]').trigger("click");
      expect(wrapper.find("#port-edit-53-udp-name").exists()).toBe(true);
      expect(wrapper.find("#port-edit-53-udp-audience-all").exists()).toBe(true);
      expect(wrapper.find("#port-edit-53-audience-all").exists()).toBe(false);
    });

    it("Save sends the new name with proto: udp and leaves the TCP entry", async () => {
      api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
      const { wrapper } = mountPorts();
      await wrapper.get('[data-port-edit="53/udp"]').trigger("click");
      await wrapper.get('[data-port-edit-name="53/udp"]').setValue("resolver");
      await wrapper.get("[data-port-edit-save]").trigger("click");
      await flushPromises();
      expect(lastPublished()).toEqual([TCP_ENTRY, { port: 53, proto: "udp", name: "resolver", for: "all" }]);
      expect(wrapper.find("[data-port-edit-save]").exists()).toBe(false);
    });

    it("Save with selected members sends their ids on the UDP entry only", async () => {
      api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
      const { wrapper } = mountPorts();
      await wrapper.get('[data-port-edit="53/udp"]').trigger("click");
      await wrapper.get("#port-edit-53-udp-audience-nodes").setValue();
      await wrapper.get('#port-edit-53-udp-audience-list [data-port-audience-node="' + PEER_FULL + '"]').setValue(true);
      await wrapper.get("[data-port-edit-save]").trigger("click");
      await flushPromises();
      expect(lastPublished()).toEqual([TCP_ENTRY, { ...UDP_ENTRY, for: [PEER_FULL] }]);
    });

    it("Cancel closes the editor without a request", async () => {
      const { wrapper } = mountPorts();
      await wrapper.get('[data-port-edit="53/udp"]').trigger("click");
      await wrapper.get("[data-port-edit-cancel]").trigger("click");
      expect(wrapper.find("[data-port-edit-save]").exists()).toBe(false);
      expect(api).not.toHaveBeenCalled();
    });
  });

  describe("Edit on the TCP row", () => {
    it("opens an editor titled \"Edit port 53\" with the id prefix port-edit-53-", async () => {
      const { wrapper } = mountPorts();
      await wrapper.get('[data-port-edit="53"]').trigger("click");
      expect(wrapper.get(".port-edit h3").text()).toBe("Edit port 53");
      expect(wrapper.get('[data-port-edit-name="53"]').element.value).toBe("dns-tcp");
      expect(wrapper.find("#port-edit-53-audience-all").exists()).toBe(true);
      expect(wrapper.find("#port-edit-53-udp-audience-all").exists()).toBe(false);
    });

    it("Save sends the new name without a proto field and leaves the UDP entry", async () => {
      api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
      const { wrapper } = mountPorts();
      await wrapper.get('[data-port-edit="53"]').trigger("click");
      await wrapper.get('[data-port-edit-name="53"]').setValue("web-dns");
      await wrapper.get("[data-port-edit-save]").trigger("click");
      await flushPromises();
      expect(lastPublished()).toEqual([{ port: 53, name: "web-dns", for: "all" }, UDP_ENTRY]);
      expect("proto" in lastPublished()[0]).toBe(false);
    });
  });

  describe("the editor of the other protocol", () => {
    it("opening the UDP editor closes the TCP one: only one is open", async () => {
      const { wrapper } = mountPorts();
      await wrapper.get('[data-port-edit="53"]').trigger("click");
      await wrapper.get('[data-port-edit="53/udp"]').trigger("click");
      expect(wrapper.findAll("[data-port-edit-save]")).toHaveLength(1);
      expect(wrapper.find('[data-port-edit-name="53"]').exists()).toBe(false);
      expect(wrapper.find('[data-port-edit-name="53/udp"]').exists()).toBe(true);
    });

    it("opening the TCP editor closes the UDP one", async () => {
      const { wrapper } = mountPorts();
      await wrapper.get('[data-port-edit="53/udp"]').trigger("click");
      await wrapper.get('[data-port-edit="53"]').trigger("click");
      expect(wrapper.findAll("[data-port-edit-save]")).toHaveLength(1);
      expect(wrapper.find('[data-port-edit-name="53/udp"]').exists()).toBe(false);
      expect(wrapper.find('[data-port-edit-name="53"]').exists()).toBe(true);
    });
  });
});

describe("publishing through the form", () => {
  const mountWith = (published) => (mounted = mountSection(PortsSection, {
    props: { slot: "main" },
    state: baseState({ ports: { published: published.map((p) => ({ ...p, local: "ok" })), network: [] } }),
    config: baseConfig({ ports: { published, opened: [] } }),
  }));
  const publishVia = async (wrapper, port, proto) => {
    await wrapper.get("#port-add-port").setValue(port);
    await wrapper.get("#port-add-name").setValue("dns");
    await wrapper.get("#port-add-proto").setValue(proto);
    await wrapper.get("#port-add-btn").trigger("click");
    await flushPromises();
  };

  describe("UDP 53 while TCP 53 is published", () => {
    it("PUTs two entries: the TCP one untouched, the new one with proto: udp", async () => {
      api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
      const { wrapper } = mountWith([TCP_ENTRY]);
      await publishVia(wrapper, "53", "udp");
      expect(lastPublished()).toEqual([TCP_ENTRY, { port: 53, proto: "udp", name: "dns", for: "all" }]);
      expect(wrapper.get("#port-add-error").text()).toBe("");
    });
  });

  describe("UDP 53 again", () => {
    it("shows \"UDP port 53 is already published.\" and sends nothing", async () => {
      const { wrapper } = mountWith([TCP_ENTRY, UDP_ENTRY]);
      await publishVia(wrapper, "53", "udp");
      expect(wrapper.get("#port-add-error").text()).toBe("UDP port 53 is already published.");
      expect(api).not.toHaveBeenCalled();
    });
  });

  describe("TCP 53 again", () => {
    it("shows the former \"Port 53 is already published.\" and sends nothing", async () => {
      const { wrapper } = mountWith([TCP_ENTRY, UDP_ENTRY]);
      await publishVia(wrapper, "53", "tcp");
      expect(wrapper.get("#port-add-error").text()).toBe("Port 53 is already published.");
      expect(api).not.toHaveBeenCalled();
    });
  });

  describe("TCP 8080", () => {
    it("PUTs an entry without a proto field", async () => {
      api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
      const { wrapper } = mountWith([UDP_ENTRY]);
      await publishVia(wrapper, "8080", "tcp");
      expect(lastPublished()).toEqual([UDP_ENTRY, { port: 8080, name: "dns", for: "all" }]);
      expect("proto" in lastPublished()[1]).toBe(false);
    });
  });

  describe("a repeated port error", () => {
    it("goes away after a successful publish", async () => {
      api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
      const { wrapper } = mountWith([UDP_ENTRY]);
      await publishVia(wrapper, "53", "udp");
      expect(wrapper.get("#port-add-error").text()).toBe("UDP port 53 is already published.");
      await publishVia(wrapper, "54", "udp");
      expect(wrapper.get("#port-add-error").text()).toBe("");
    });
  });
});
