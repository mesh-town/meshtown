/* The Shared services panel of one network of several, with the two protocols: publishing, editing and unpublishing a UDP
 * port writes `proto: "udp"` into the config of that network only and never touches the TCP entry of the same number, and a
 * local number taken by one protocol is not reported as busy for the other. The fake node has two networks:
 * A = "friends" (the main slot), B = "my devices". */

import { mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createFakeNode } from "../testing/fake-node.mjs";
import { NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, polls, settle, unmountPage } from "../testing/page.mjs";
import { SELF_FULL } from "../testing/section.mjs";
import PublishForm from "./PublishForm.vue";

const PUT_A = "/api/config";
const PUT_B = "/api/net/" + NET_B + "/config";
const ALICE = "a11ce".padEnd(64, "1");
const PEER_ONE = "B".repeat(64);
const TCP = { port: 53, name: "dns-tcp", for: "all" };
const UDP = { port: 53, proto: "udp", name: "dns-udp", for: "all" };

describe("UDP in several networks", () => {
  let form;
  afterEach(() => (form ? form.unmount() : unmountPage()));
  beforeEach(() => { form = null; });

  describe("two networks", () => {
    describe("publishing", () => {
      it("writes a UDP entry into the open network only and leaves its TCP entry of the same number alone", async () => {
        const { wrapper, node } = await setup(NET_B, { a: { own: [TCP] }, b: { own: [TCP] } });
        await when_publishing(wrapper, 53, "dns-udp", "udp");
        expect(putPaths(node)).toEqual([PUT_B]);
        expect(publishedOf(node, PUT_B)).toEqual([TCP, UDP]);
      });

      it("a TCP port of that number in the other network does not block UDP here", async () => {
        const { wrapper, node } = await setup(NET_B, { a: { own: [TCP] } });
        await when_publishing(wrapper, 53, "dns-udp", "udp");
        expect(putPaths(node)).toEqual([PUT_B]);
        expect(publishedOf(node, PUT_B)).toEqual([UDP]);
      });

      it("a TCP publication of the same number still carries no proto field", async () => {
        const { wrapper, node } = await setup(NET_B, { b: { own: [UDP] } });
        await when_publishing(wrapper, 53, "dns-tcp", "tcp");
        const written = publishedOf(node, PUT_B);
        expect(written).toHaveLength(2);
        expect("proto" in written.find((p) => p.name === "dns-tcp")).toBe(false);
        expect(written.find((p) => p.name === "dns-udp").proto).toBe("udp");
      });

      it("in the main network goes to the main config", async () => {
        const { wrapper, node } = await setup("main", { b: { own: [TCP] } });
        await when_publishing(wrapper, 53, "dns-udp", "udp");
        expect(putPaths(node)).toEqual([PUT_A]);
        expect(publishedOf(node, PUT_A)).toEqual([UDP]);
      });
    });

    describe("editing", () => {
      it("the UDP name changes with proto kept in the open network only, and the TCP entries stay", async () => {
        const { wrapper, node } = await setup(NET_B, { a: { own: [TCP, UDP] }, b: { own: [TCP, UDP] } });
        await wrapper.get('[data-port-edit="53/udp"]').trigger("click");
        await settle();
        await wrapper.get('[data-port-edit-name="53/udp"]').setValue("resolver");
        await wrapper.get("[data-port-edit-save]").trigger("click");
        await settle();
        expect(putPaths(node)).toEqual([PUT_B]);
        expect(publishedOf(node, PUT_B)).toEqual([TCP, { port: 53, proto: "udp", name: "resolver", for: "all" }]);
      });

      it("the editor of the UDP row is filled from the UDP audience, not the TCP one of the same number", async () => {
        const { wrapper } = await setup(NET_B, { b: { own: [{ ...TCP, for: [ALICE] }, UDP] } });
        await wrapper.get('[data-port-edit="53/udp"]').trigger("click");
        await settle();
        expect(wrapper.get("#port-edit-53-udp-audience-all").element.checked).toBe(true);
        expect(wrapper.find("#port-edit-53-udp-audience-list").exists()).toBe(false);
      });
    });

    describe("unpublishing", () => {
      it("removes the UDP entry from the open network only; its TCP entry and the other network stay", async () => {
        const { wrapper, node } = await setup(NET_B, { a: { own: [TCP, UDP] }, b: { own: [TCP, UDP] } });
        await wrapper.get('[data-port-unpublish="53/udp"]').trigger("click");
        await settle();
        expect(putPaths(node)).toEqual([PUT_B]);
        expect(publishedOf(node, PUT_B)).toEqual([TCP]);
      });
    });

    describe("local numbers already taken", () => {
      it("TCP 53 opened on 2053 in one network: UDP 53 of the other network is not reported as busy", async () => {
        const { wrapper } = await setup(NET_B, {
          a: { rows: [netRow(PEER_ONE, 53, null, listening(2053))], opened: [{ node: PEER_ONE, port: 53, local: 2053 }] },
          b: { rows: [netRow(ALICE, 53, "udp")] },
        });
        const udpRow = wrapper.get('[data-port-row$="/53/udp"]');
        expect(flat(udpRow)).not.toContain("is already used");
        expect(udpRow.get("input[type=number]").element.value).toBe("2053");
      });

      it("the same TCP number is reported as busy on a TCP row of the other network", async () => {
        const { wrapper } = await setup(NET_B, {
          a: { rows: [netRow(PEER_ONE, 53, null, listening(2053))], opened: [{ node: PEER_ONE, port: 53, local: 2053 }] },
          b: { rows: [netRow(ALICE, 53, null)] },
        });
        const tcpRow = rowsOf(wrapper).find((r) => r.text().includes("alice"));
        expect(flat(tcpRow)).toContain("2053 is already used by peer-one (friends)");
        expect(tcpRow.get("input[type=number]").element.value).toBe("2054");
      });

      it("UDP 53 opened on 2053 in one network: TCP 53 of the other network is not reported as busy", async () => {
        const { wrapper } = await setup(NET_B, {
          a: { rows: [netRow(PEER_ONE, 53, "udp", listening(2053))], opened: [{ node: PEER_ONE, port: 53, proto: "udp", local: 2053 }] },
          b: { rows: [netRow(ALICE, 53, null)] },
        });
        const tcpRow = rowsOf(wrapper).find((r) => r.text().includes("alice"));
        expect(flat(tcpRow)).not.toContain("is already used");
        expect(tcpRow.get("input[type=number]").element.value).toBe("2053");
      });

      it("UDP 53 opened on 2053 in one network: UDP 53 of the other network is reported as busy", async () => {
        const { wrapper } = await setup(NET_B, {
          a: { rows: [netRow(PEER_ONE, 53, "udp", listening(2053))], opened: [{ node: PEER_ONE, port: 53, proto: "udp", local: 2053 }] },
          b: { rows: [netRow(ALICE, 53, "udp")] },
        });
        const udpRow = rowsOf(wrapper).find((r) => r.text().includes("alice"));
        expect(flat(udpRow)).toContain("2053 is already used by peer-one (friends)");
      });
    });
  });

  describe("PublishForm after a successful publish", () => {
    it("the protocol is TCP again", async () => {
      const publish = vi.fn(async () => true);
      form = mount(PublishForm, { props: { busy: false, peers: [], publish }, attachTo: document.body });
      await form.get("#port-add-port").setValue("53");
      await form.get("#port-add-proto").setValue("udp");
      await form.get("#port-add-btn").trigger("click");
      expect(publish).toHaveBeenCalledWith(53, "", "all", "udp");
      expect(form.get("#port-add-proto").element.value).toBe("tcp");
    });

    it("a refused publish keeps UDP selected", async () => {
      const publish = vi.fn(async () => false);
      form = mount(PublishForm, { props: { busy: false, peers: [], publish }, attachTo: document.body });
      await form.get("#port-add-port").setValue("53");
      await form.get("#port-add-proto").setValue("udp");
      await form.get("#port-add-btn").trigger("click");
      expect(form.get("#port-add-proto").element.value).toBe("udp");
    });
  });
});

// Setup and helpers.

const listening = (local) => ({ local, state: "listening", error: null });
const netRow = (node, port, proto, opened = null) => ({
  node, node_name: node === ALICE ? "alice" : "peer-one", online: true, port, name: "svc" + port, published: true, opened,
  ...(proto ? { proto } : {}),
});
const flat = (wrapper) => (wrapper.text ? wrapper.text() : wrapper.textContent).replace(/\s+/g, " ").trim();
const rowsOf = (wrapper) => wrapper.findAll("#ports-net-tbody tr");

function applySide(state, config, { own = [], rows = [], opened = [], peers = null }) {
  state.ports = { published: own.map(({ port, proto, name }) => ({ port, ...(proto ? { proto } : {}), name, local: "ok" })), network: rows };
  config.ports = { published: own.map((p) => ({ ...p })), opened };
  if (peers) state.peers = peers;
}

const peersOfB = () => [
  { self: true, online: true, id: "aaaa1111", id_full: SELF_FULL, name: "finch-unwary", country: "RU" },
  { id: "a11ce000", id_full: ALICE, name: "alice", country: "DE", online: true },
];

/** The page on a node with two networks, opened on Shared services of `slot`, the slot configs loaded. */
async function setup(slot, { a = {}, b = {} } = {}) {
  const opts = twoNets();
  applySide(opts.state, opts.config, a);
  applySide(opts.slots[1].state, opts.slots[1].config, { peers: peersOfB(), ...b });
  const node = createFakeNode(opts);
  const { wrapper } = await mountPage(node, { hash: "peers/" + slot + "/services" });
  await polls(node, 1);
  return { wrapper, node };
}

const puts = (node) => node.calls.filter((c) => c.method === "PUT");
const putPaths = (node) => puts(node).map((c) => c.path);
const publishedOf = (node, path) => puts(node).filter((c) => c.path === path).at(-1).body.config.ports.published;

/** Fills the publish form with the given protocol and presses "Publish port"; the audience stays "All members". */
async function when_publishing(wrapper, port, name, proto) {
  await wrapper.get("#port-add-port").setValue(String(port));
  await wrapper.get("#port-add-name").setValue(name);
  await wrapper.get("#port-add-proto").setValue(proto);
  await wrapper.get("#port-add-btn").trigger("click");
  await settle();
}
