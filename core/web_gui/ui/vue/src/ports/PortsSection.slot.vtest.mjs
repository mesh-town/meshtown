/* Shared services of one network: every write goes to the config of the network the panel is opened on (the main one to
 * `/api/config`, another to `/api/net/<slot>/config`), a refusal is shown in the panel and the other network is not written.
 * The page runs on a fake node with two networks: A = "friends" (the main slot), B = "my devices". */

import { afterEach, describe, expect, it } from "vitest";
import { createFakeNode } from "../testing/fake-node.mjs";
import { NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, polls, settle, unmountPage } from "../testing/page.mjs";
import { SELF_FULL } from "../testing/section.mjs";

const PUT_A = "/api/config";
const PUT_B = "/api/net/" + NET_B + "/config";
const ALICE = "a11ce".padEnd(64, "1");
const PEER_ONE = "B".repeat(64);
const REFUSAL = { error: "conflict", message: "the node refused this change" };

afterEach(unmountPage);

describe("Shared services of the second network", () => {
  describe("publish", () => {
    it("publish writes only the slot config", async () => {
      const { wrapper, node } = await setup(NET_B);
      await when_publishing(wrapper, 9090, "db");
      expect(putPaths(node)).toEqual([PUT_B]);
      expect(publishedOf(node, PUT_B)).toContainEqual({ port: 9090, name: "db", for: "all" });
    });

    it("the audience choice lists the members of this network only", async () => {
      const { wrapper } = await setup(NET_B);
      await wrapper.get("#port-add-audience-nodes").setValue();
      const ids = wrapper.findAll("#port-add-audience-list [data-port-audience-node]").map((b) => b.attributes("data-port-audience-node"));
      expect(ids).toEqual([ALICE]);
    });

    it("a chosen member goes into the list of this network as its full id", async () => {
      const { wrapper, node } = await setup(NET_B);
      await wrapper.get("#port-add-port").setValue("9090");
      await wrapper.get("#port-add-audience-nodes").setValue();
      await wrapper.get('[data-port-audience-node="' + ALICE + '"]').setValue(true);
      await wrapper.get("#port-add-btn").trigger("click");
      await settle();
      expect(publishedOf(node, PUT_B)).toContainEqual({ port: 9090, name: "", for: [ALICE] });
    });

    it("an empty member choice is an error under the form and no request", async () => {
      const { wrapper, node } = await setup(NET_B);
      await wrapper.get("#port-add-port").setValue("9090");
      await wrapper.get("#port-add-audience-nodes").setValue();
      await wrapper.get("#port-add-btn").trigger("click");
      await settle();
      expect(wrapper.get("#port-add-error").text()).toBe("Select at least one member.");
      expect(putPaths(node)).toEqual([]);
    });

    it("the button reads \"Publish port\"", async () => {
      const { wrapper } = await setup(NET_B);
      expect(wrapper.get("#port-add-btn").text()).toBe("Publish port");
    });
  });

  describe("refused publish", () => {
    it("refused publish: error, other net untouched", async () => {
      const { wrapper, node } = await setup(NET_B);
      const mainBefore = structuredClone(node.config);
      node.fail = { path: PUT_B, status: 409, body: REFUSAL };
      await when_publishing(wrapper, 9090, "db");
      const status = wrapper.get("#ports-status");
      expect(status.text()).toMatch(/^Couldn't save: /);
      expect(status.text()).toContain(REFUSAL.message);
      expect(status.classes()).toContain("field-error-text");
      expect(putPaths(node)).toEqual([PUT_B]);
      expect(node.config, "the main config stays as it was").toEqual(mainBefore);
    });

    it("the form keeps what was typed so the user can retry", async () => {
      const { wrapper, node } = await setup(NET_B);
      node.fail = { path: PUT_B, status: 409, body: REFUSAL };
      await when_publishing(wrapper, 9090, "db");
      expect(wrapper.get("#port-add-port").element.value).toBe("9090");
      expect(wrapper.get("#port-add-name").element.value).toBe("db");
    });

    it("a refused edit is shown in the editor, the editor stays open and the other network is not written", async () => {
      const { wrapper, node } = await setup(NET_B, { b: { own: [own(8080, "web", "all")] } });
      await wrapper.get('[data-port-edit="8080"]').trigger("click");
      await wrapper.get('[data-port-edit-name="8080"]').setValue("site");
      node.fail = { path: PUT_B, status: 409, body: REFUSAL };
      await wrapper.get("[data-port-edit-save]").trigger("click");
      await settle();
      expect(wrapper.get(".port-edit [role=\"alert\"]").text()).toMatch(/^Couldn't save: /);
      expect(wrapper.find("[data-port-edit-save]").exists()).toBe(true);
      expect(putPaths(node)).toEqual([PUT_B]);
    });

    it("the refusal goes away after a successful publish", async () => {
      const { wrapper, node } = await setup(NET_B);
      node.fail = { path: PUT_B, status: 409, body: REFUSAL };
      await when_publishing(wrapper, 9090, "db");
      node.fail = null;
      await wrapper.get("#port-add-btn").trigger("click");
      await settle();
      expect(wrapper.get("#ports-status").text()).toBe("Saved");
      expect(wrapper.get("#ports-status").classes()).not.toContain("field-error-text");
    });
  });

  describe("edit and unpublish", () => {
    it("the name changes in the config of this network only", async () => {
      const { wrapper, node } = await setup(NET_B, { a: { own: [own(8080, "web", "all")] }, b: { own: [own(8080, "web", "all")] } });
      await wrapper.get('[data-port-edit="8080"]').trigger("click");
      await wrapper.get('[data-port-edit-name="8080"]').setValue("site");
      await wrapper.get("[data-port-edit-save]").trigger("click");
      await settle();
      expect(putPaths(node)).toEqual([PUT_B]);
      expect(publishedOf(node, PUT_B)).toContainEqual({ port: 8080, name: "site", for: "all" });
    });

    it("the editor offers the members of this network only", async () => {
      const { wrapper } = await setup(NET_B, { b: { own: [own(8080, "web", "all")] } });
      await wrapper.get('[data-port-edit="8080"]').trigger("click");
      await wrapper.get("#port-edit-8080-audience-nodes").setValue();
      const ids = wrapper.findAll("#port-edit-8080-audience-list [data-port-audience-node]").map((b) => b.attributes("data-port-audience-node"));
      expect(ids).toEqual([ALICE]);
    });

    it("unpublish removes the port from this network and leaves the same port of the other one", async () => {
      const { wrapper, node } = await setup(NET_B, { a: { own: [own(8080, "web", "all")] }, b: { own: [own(8080, "web", "all")] } });
      await wrapper.get('[data-port-unpublish="8080"]').trigger("click");
      await settle();
      expect(putPaths(node)).toEqual([PUT_B]);
      expect(publishedOf(node, PUT_B).map((p) => p.port)).not.toContain(8080);
    });
  });

  describe("open", () => {
    it("Open writes ports.opened of B", async () => {
      const { wrapper, node } = await setup(NET_B, { b: { rows: [netRow(ALICE, "alice", 9000)] } });
      await openButtonOf(wrapper, "alice").trigger("click");
      await settle();
      expect(putPaths(node)).toEqual([PUT_B]);
      expect(openedOf(node, PUT_B)).toEqual([{ node: ALICE, port: 9000, local: 11000 }]);
    });

    it("Close removes the pair from ports.opened of B only", async () => {
      const opened = { node: ALICE, port: 9000, local: 11000 };
      const { wrapper, node } = await setup(NET_B, { a: { opened: [{ node: PEER_ONE, port: 80, local: 2080 }] }, b: { rows: [netRow(ALICE, "alice", 9000, listening(11000))], opened: [opened] } });
      await rowOf(wrapper, "alice").get("[data-port-close]").trigger("click");
      await settle();
      expect(putPaths(node)).toEqual([PUT_B]);
      expect(openedOf(node, PUT_B)).toEqual([]);
    });
  });
});

describe("Shared services of the main network", () => {
  it("publish writes only /api/config", async () => {
    const { wrapper, node } = await setup("main");
    await when_publishing(wrapper, 9090, "db");
    expect(putPaths(node)).toEqual([PUT_A]);
    expect(publishedOf(node, PUT_A)).toContainEqual({ port: 9090, name: "db", for: "all" });
  });

  it("the audience choice lists the members of the main network, not of the second one", async () => {
    const { wrapper } = await setup("main");
    await wrapper.get("#port-add-audience-nodes").setValue();
    const ids = wrapper.findAll("#port-add-audience-list [data-port-audience-node]").map((b) => b.attributes("data-port-audience-node"));
    expect(ids).toEqual([PEER_ONE, "C".repeat(64)]);
  });

  it("Open writes ports.opened of the main network only", async () => {
    const { wrapper, node } = await setup("main", { a: { rows: [netRow(PEER_ONE, "peer-one", 80)] } });
    await openButtonOf(wrapper, "peer-one").trigger("click");
    await settle();
    expect(putPaths(node)).toEqual([PUT_A]);
    expect(openedOf(node, PUT_A)).toEqual([{ node: PEER_ONE, port: 80, local: 2080 }]);
  });
});

// Setup and helpers.

const own = (port, name, audience) => ({ port, name, for: audience });
const listening = (local) => ({ local, state: "listening", error: null });
const netRow = (node, nodeName, port, opened = null) => ({ node, node_name: nodeName, online: true, port, name: "svc" + port, published: true, opened });

/** One side of the fake node (`own`: published ports, `rows`: network ports, `opened`: the config's opened list, `peers`). */
function applySide(state, config, { own: published = [], rows = [], opened = [], peers = null }) {
  state.ports = { published: published.map(({ port, name }) => ({ port, name, local: "ok" })), network: rows };
  config.ports = { published: published.map((p) => ({ port: p.port, name: p.name, for: p.for })), opened };
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
const lastBody = (node, path) => puts(node).filter((c) => c.path === path).at(-1).body;
const publishedOf = (node, path) => lastBody(node, path).config.ports.published;
const openedOf = (node, path) => lastBody(node, path).config.ports.opened;

const rowOf = (wrapper, text) => wrapper.findAll("#ports-net-tbody tr").find((r) => r.text().includes(text));
const openButtonOf = (wrapper, text) => rowOf(wrapper, text).findAll("button").find((b) => b.text() === "Open on this computer");

/** Fills the publish form (audience stays "All members") and presses "Publish port". */
async function when_publishing(wrapper, port, name) {
  await wrapper.get("#port-add-port").setValue(String(port));
  await wrapper.get("#port-add-name").setValue(name);
  await wrapper.get("#port-add-btn").trigger("click");
  await settle();
}
