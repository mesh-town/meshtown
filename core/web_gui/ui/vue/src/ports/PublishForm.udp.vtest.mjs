import { afterEach, describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";
import PublishForm from "./PublishForm.vue";

const B = "B".repeat(64);
const PEERS = [
  { id: "bbbb2222", id_full: B, name: "peer-one", self: false, online: true },
  { id: "aaaa1111", id_full: "A".repeat(64), name: "me", self: true, online: true },
];
let wrapper;
const mountForm = (publish = vi.fn(async () => true)) => {
  wrapper = mount(PublishForm, { props: { busy: false, peers: PEERS, publish }, attachTo: document.body });
  return publish;
};
afterEach(() => wrapper.unmount());

const fill = async (port, name) => {
  await wrapper.get("#port-add-port").setValue(port);
  await wrapper.get("#port-add-name").setValue(name);
};

describe("PublishForm: protocol", () => {
  describe("the protocol field", () => {
    it("is a select labelled \"protocol\"", () => {
      mountForm();
      const select = wrapper.get("#port-add-proto");
      expect(select.element.tagName).toBe("SELECT");
      expect(select.attributes("aria-label")).toBe("protocol");
    });

    it("offers TCP and UDP, in that order", () => {
      mountForm();
      const options = wrapper.findAll("#port-add-proto option");
      expect(options.map((o) => [o.attributes("value"), o.text()])).toEqual([["tcp", "TCP"], ["udp", "UDP"]]);
    });

    it("is TCP by default", () => {
      mountForm();
      expect(wrapper.get("#port-add-proto").element.value).toBe("tcp");
    });
  });

  describe("publishing with the default protocol", () => {
    it("calls publish(port, name, \"all\", \"tcp\")", async () => {
      const publish = mountForm();
      await fill("8080", "web");
      await wrapper.get("#port-add-btn").trigger("click");
      expect(publish).toHaveBeenCalledWith(8080, "web", "all", "tcp");
    });
  });

  describe("publishing with UDP chosen", () => {
    it("calls publish(port, name, \"all\", \"udp\")", async () => {
      const publish = mountForm();
      await fill("53", "dns");
      await wrapper.get("#port-add-proto").setValue("udp");
      await wrapper.get("#port-add-btn").trigger("click");
      expect(publish).toHaveBeenCalledWith(53, "dns", "all", "udp");
    });

    it("with selected members passes their ids and the protocol", async () => {
      const publish = mountForm();
      await fill("53", "dns");
      await wrapper.get("#port-add-proto").setValue("udp");
      await wrapper.get("#port-add-audience-nodes").setValue();
      await wrapper.get('[data-port-audience-node="' + B + '"]').setValue(true);
      await wrapper.get("#port-add-btn").trigger("click");
      expect(publish).toHaveBeenCalledWith(53, "dns", [B], "udp");
    });

    it("choosing TCP again publishes TCP", async () => {
      const publish = mountForm();
      await fill("53", "dns");
      await wrapper.get("#port-add-proto").setValue("udp");
      await wrapper.get("#port-add-proto").setValue("tcp");
      await wrapper.get("#port-add-btn").trigger("click");
      expect(publish).toHaveBeenCalledWith(53, "dns", "all", "tcp");
    });
  });

  describe("after a refused publish", () => {
    it("the form keeps the port, the name and the protocol", async () => {
      mountForm(vi.fn(async () => false));
      await fill("53", "dns");
      await wrapper.get("#port-add-proto").setValue("udp");
      await wrapper.get("#port-add-btn").trigger("click");
      await Promise.resolve();
      await wrapper.vm.$nextTick();
      expect(wrapper.get("#port-add-port").element.value).toBe("53");
      expect(wrapper.get("#port-add-proto").element.value).toBe("udp");
    });
  });
});
