import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api, ApiError } from "../core/api.ts";
import { baseConfig, baseState, mountSection } from "../testing/section.mjs";
import PortsSection from "./PortsSection.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const OWN = [{ port: 8080, name: "web", for: "all", local: "ok" }, { port: 9090, name: "db", for: "all", local: "ok" }];
let mounted;

describe("PortsSection: the node refuses an edit of a published port", () => {
  describe("a refusal while saving the editor", () => {
    it("the refusal text is shown in the editor's alert and the editor stays open", async () => {
      const wrapper = await refuseEdit();
      const alert = wrapper.get(".port-edit [role=\"alert\"]");
      expect(alert.text()).toMatch(/^Couldn't save: /);
      expect(alert.attributes("hidden")).toBeUndefined();
      expect(wrapper.find("[data-port-edit-save]").exists()).toBe(true);
    });

    it("the shared #ports-status does not show the refusal", async () => {
      const wrapper = await refuseEdit();
      const status = wrapper.get("#ports-status");
      expect(status.text()).not.toContain("Couldn't save");
      expect(status.classes()).not.toContain("field-error-text");
    });
  });

  describe("a refusal while publishing a new port (unchanged)", () => {
    it("the refusal text stays in #ports-status", async () => {
      const wrapper = mountPorts().wrapper;
      api.mockRejectedValue(new ApiError(500, "boom", "disk is full"));
      await wrapper.get("#port-add-port").setValue("8081");
      await wrapper.get("#port-add-btn").trigger("click");
      await flushPromises();
      const status = wrapper.get("#ports-status");
      expect(status.text()).toMatch(/^Couldn't save: /);
      expect(status.classes()).toContain("field-error-text");
    });
  });

  describe("a successful edit (unchanged)", () => {
    it("\"Saved\" in #ports-status and the editor closes", async () => {
      api.mockImplementation(async (_m, _p, body) => ({ config: body.config, rev: 2 }));
      const { wrapper } = mountPorts();
      await wrapper.get('[data-port-edit="8080"]').trigger("click");
      await wrapper.get('[data-port-edit-name="8080"]').setValue("site");
      await wrapper.get("[data-port-edit-save]").trigger("click");
      await flushPromises();
      expect(wrapper.find("[data-port-edit-save]").exists()).toBe(false);
      expect(wrapper.get("#ports-status").text()).toBe("Saved");
    });
  });
});

afterEach(() => { mounted.wrapper.unmount(); api.mockReset(); });

function mountPorts() {
  mounted = mountSection(PortsSection, {
    state: baseState({ ports: { published: OWN, network: [] } }),
    config: baseConfig({ ports: { published: OWN.map(({ port, name, for: f }) => ({ port, name, for: f })), opened: [] } }),
  });
  return mounted;
}

// Opens the editor of port 8080, renames it and saves while the node answers 500.
async function refuseEdit() {
  const { wrapper } = mountPorts();
  api.mockRejectedValue(new ApiError(500, "boom", "disk is full"));
  await wrapper.get('[data-port-edit="8080"]').trigger("click");
  await wrapper.get('[data-port-edit-name="8080"]').setValue("site");
  await wrapper.get("[data-port-edit-save]").trigger("click");
  await flushPromises();
  return wrapper;
}
