import { afterEach, describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";
import PublishForm from "./PublishForm.vue";

const B = "B".repeat(64);
const C = "C".repeat(64);
const PEERS = [
  { id: "bbbb2222", id_full: B, name: "peer-one", self: false, online: true },
  { id: "aaaa1111", id_full: "A".repeat(64), name: "me", self: true, online: true },
  { id: "cccc3333", id_full: C, name: "", self: false, online: false },
];
let wrapper;
const mountForm = (peers = PEERS, publish = vi.fn(async () => true)) => {
  wrapper = mount(PublishForm, { props: { busy: false, peers, publish }, attachTo: document.body });
  return publish;
};
afterEach(() => wrapper.unmount());

describe("PublishForm", () => {
  it("by default \"All members\", no node list", () => {
    mountForm();
    expect(wrapper.get("#port-add-audience-all").element.checked).toBe(true);
    expect(wrapper.find("#port-add-audience-list").exists()).toBe(false);
  });

  it("\"Selected nodes\" — checkboxes from peers without self, title is the full id, offline with a mark", async () => {
    mountForm();
    await wrapper.get("#port-add-audience-nodes").setValue();
    const rows = wrapper.findAll("#port-add-audience-list .port-audience-row");
    expect(rows).toHaveLength(2);
    expect(rows[0].attributes("title")).toBe(B);
    expect(rows[0].text()).toContain("peer-one");
    expect(rows[0].text()).not.toContain("offline");
    expect(rows[1].text()).toContain("offline");
    expect(wrapper.find('[data-port-audience-node="' + "A".repeat(64) + '"]').exists()).toBe(false);
  });

  it("check a node and \"Publish\" — publish(port, name, [id_full])", async () => {
    const publish = mountForm();
    await wrapper.get("#port-add-port").setValue("8081");
    await wrapper.get("#port-add-name").setValue("web");
    await wrapper.get("#port-add-audience-nodes").setValue();
    await wrapper.get('[data-port-audience-node="' + B + '"]').setValue(true);
    await wrapper.get("#port-add-btn").trigger("click");
    expect(publish).toHaveBeenCalledWith(8081, "web", [B]);
  });

  it("success — the form is reset to \"All members\"", async () => {
    mountForm();
    await wrapper.get("#port-add-port").setValue("8081");
    await wrapper.get("#port-add-audience-nodes").setValue();
    await wrapper.get('[data-port-audience-node="' + B + '"]').setValue(true);
    await wrapper.get("#port-add-btn").trigger("click");
    await Promise.resolve();
    await wrapper.vm.$nextTick();
    expect(wrapper.get("#port-add-port").element.value).toBe("");
    expect(wrapper.get("#port-add-audience-all").element.checked).toBe(true);
    expect(wrapper.find("#port-add-audience-list").exists()).toBe(false);
  });

  it("no other members — a hint", async () => {
    mountForm([PEERS[1]]);
    await wrapper.get("#port-add-audience-nodes").setValue();
  expect(wrapper.get("#port-add-audience-list").text()).toContain("There are no other members in the network yet");
  });
});
