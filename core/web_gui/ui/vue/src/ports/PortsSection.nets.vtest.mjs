/* "Ports" with several networks: "My published ports" merged by port number with "Shared with" per network, the publish form
 * and the editor with a group per network, writes into the config of each network's own slot, "Open" that skips a local number
 * already taken in any network, errors shown at the row. One network keeps the former look and requests.
 * The whole page runs on a fake node with two networks: A = "friends" (the main slot), B = "my devices". */

import { DOMWrapper } from "@vue/test-utils";
import { afterEach, describe, expect, it } from "vitest";
import { demoConfig, demoState } from "../testing/demo-node.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, openTab, polls, settle, unmountPage } from "../testing/page.mjs";
import { SELF_FULL } from "../testing/section.mjs";

const NET_A_NAME = "friends";
const NET_B_NAME = "my devices";
const PUT_A = "/api/config";
const PUT_B = "/api/net/" + NET_B + "/config";
const ALICE = "a11ce".padEnd(64, "1");
const BOB = "b0b".padEnd(64, "2");
const PEER_ONE = "B".repeat(64);

afterEach(unmountPage);

describe("two networks", () => {
  describe("my published ports", () => {
    it("a port published in both is one row, with \"Shared with\" listing each network", async () => {
      const { wrapper } = await setup({
        a: { own: [own(8080, "web", "all")] },
        b: { own: [own(8080, "web", [ALICE])] },
      });
      const rows = wrapper.findAll("#ports-own-tbody tr");
      expect(rows).toHaveLength(1);
      expect(flat(rows[0])).toContain("8080");
      expect(flat(rows[0])).toContain(NET_A_NAME + ": all · " + NET_B_NAME + ": alice");
    });

    it("a port published only in the second network is a row that names only that network", async () => {
      const { wrapper } = await setup({ b: { own: [own(9090, "db", "all")] } });
      const rows = wrapper.findAll("#ports-own-tbody tr");
      expect(rows).toHaveLength(1);
      expect(flat(rows[0])).toContain(NET_B_NAME + ": all");
      expect(flat(rows[0])).not.toContain(NET_A_NAME + ":");
    });

    it("different ports of different networks are different rows", async () => {
      const { wrapper } = await setup({ a: { own: [own(8080, "web", "all")] }, b: { own: [own(9090, "db", "all")] } });
      expect(wrapper.findAll("#ports-own-tbody tr")).toHaveLength(2);
    });
  });

  describe("publishing a new port", () => {
    it("ticked \"All members\" in both networks writes the port into the config of each slot, the main one first", async () => {
      const { wrapper, node } = await setup();
      await when_publishing(wrapper, 9090, "db", { [NET_A_NAME]: ["All members"], [NET_B_NAME]: ["All members"] });
      expect(putPaths(node)).toEqual([PUT_A, PUT_B]);
      expect(publishedOf(node, PUT_A)).toContainEqual({ port: 9090, name: "db", for: "all" });
      expect(publishedOf(node, PUT_B)).toContainEqual({ port: 9090, name: "db", for: "all" });
    });

    it("a network with nothing ticked does not get the port and its config is not written", async () => {
      const { wrapper, node } = await setup();
      await when_publishing(wrapper, 9090, "db", { [NET_A_NAME]: [], [NET_B_NAME]: ["All members"] });
      expect(putPaths(node)).toEqual([PUT_B]);
      expect(publishedOf(node, PUT_B)).toContainEqual({ port: 9090, name: "db", for: "all" });
    });

    // Member checkboxes are visible while "All members" is not ticked.
    it("ticked members of one network are written as that network's list of member ids", async () => {
      const { wrapper, node } = await setup();
      await when_publishing(wrapper, 9090, "db", { [NET_A_NAME]: ["All members"], [NET_B_NAME]: ["alice"] });
      expect(publishedOf(node, PUT_A)).toContainEqual({ port: 9090, name: "db", for: "all" });
      expect(publishedOf(node, PUT_B)).toContainEqual({ port: 9090, name: "db", for: [ALICE] });
    });

    it("nothing ticked in any network is an error under the form and no request", async () => {
      const { wrapper, node } = await setup();
      await when_publishing(wrapper, 9090, "db", { [NET_A_NAME]: [], [NET_B_NAME]: [] });
      expect(wrapper.get("#port-add-error").text()).toBe("Select at least one member.");
      expect(putPaths(node)).toEqual([]);
    });

    it("every network has its own group labelled with the network name, with its own members", async () => {
      const { wrapper } = await setup();
      const scope = scopeOf(wrapper.get("#port-add-btn").element);
      expect(groupText(scope, NET_A_NAME)).toContain("All members");
      expect(groupText(scope, NET_A_NAME)).toContain("peer-one");
      expect(groupText(scope, NET_B_NAME)).toContain("All members");
      expect(groupText(scope, NET_B_NAME)).toContain("alice");
      expect(groupText(scope, NET_B_NAME)).not.toContain("peer-one");
    });
  });

  describe("editing a published port", () => {
    it("the name changes in every network where the port is published", async () => {
      const { wrapper, node } = await setup(both8080());
      await when_editing(wrapper, 8080, async (editor) => {
        await new DOMWrapper(editor.querySelector("[data-port-edit-name]")).setValue("site");
      });
      expect(putPaths(node)).toEqual([PUT_A, PUT_B]);
      expect(publishedOf(node, PUT_A)).toContainEqual({ port: 8080, name: "site", for: "all" });
      expect(publishedOf(node, PUT_B)).toContainEqual({ port: 8080, name: "site", for: "all" });
    });

    it("unticking the only box of the second network removes the port from that network and leaves the first one alone", async () => {
      const { wrapper, node } = await setup(both8080());
      await when_editing(wrapper, 8080, async (editor) => {
        await tick(boxIn(editor, NET_B_NAME, "All members"), false);
      });
      expect(putPaths(node)).toEqual([PUT_B]);
      expect(publishedOf(node, PUT_B).map((p) => p.port)).not.toContain(8080);
    });
  });

  describe("unpublishing", () => {
    it("removes the port from the config of every network that had it, the main one first", async () => {
      const { wrapper, node } = await setup(both8080());
      await wrapper.get('[data-port-unpublish="8080"]').trigger("click");
      await settle();
      expect(putPaths(node)).toEqual([PUT_A, PUT_B]);
      expect(publishedOf(node, PUT_A).map((p) => p.port)).not.toContain(8080);
      expect(publishedOf(node, PUT_B).map((p) => p.port)).not.toContain(8080);
    });
  });

  describe("ports shared with you", () => {
    it("rows are not merged: one per network, the member is named with its network", async () => {
      const { wrapper } = await setup({
        a: { rows: [netRow(PEER_ONE, "peer-one", 80)] },
        b: { rows: [netRow(ALICE, "alice", 9000)] },
      });
      const rows = wrapper.findAll("#ports-net-tbody tr");
      expect(rows).toHaveLength(2);
      expect(flat(rows[0])).toContain("peer-one (" + NET_A_NAME + ")");
      expect(flat(rows[1])).toContain("alice (" + NET_B_NAME + ")");
    });

    it("the same member and port in both networks stay two rows", async () => {
      const { wrapper } = await setup({
        a: { rows: [netRow(PEER_ONE, "peer-one", 80)] },
        b: { rows: [netRow(PEER_ONE, "peer-one", 80)] },
      });
      const rows = wrapper.findAll("#ports-net-tbody tr");
      expect(rows).toHaveLength(2);
      expect(flat(rows[0])).toContain("peer-one (" + NET_A_NAME + ")");
      expect(flat(rows[1])).toContain("peer-one (" + NET_B_NAME + ")");
    });
  });

  describe("opening a member's port of the second network", () => {
    it("the local number 11000 is taken in the first network, so the field starts with 11001", async () => {
      const { wrapper } = await setup(taken(11000, { b: { rows: [netRow(ALICE, "alice", 9000)] } }));
      expect(localField(rowOf(wrapper, "alice")).element.value).toBe("11001");
    });

    it("\"Open\" writes the pair with 11001 into the config of the second network only", async () => {
      const { wrapper, node } = await setup(taken(11000, { b: { rows: [netRow(ALICE, "alice", 9000)] } }));
      await openButton(rowOf(wrapper, "alice")).trigger("click");
      await settle();
      expect(putPaths(node)).toEqual([PUT_B]);
      expect(openedOf(node, PUT_B)).toContainEqual({ node: ALICE, port: 9000, local: 11001 });
      expect(openedOf(node, PUT_B)).toHaveLength(1);
    });

    it("a number taken in the first network is explained under the button before the click, naming who uses it", async () => {
      const { wrapper } = await setup(taken(2080, { b: { rows: [netRow(ALICE, "alice", 80)] } }));
      const text = flat(rowOf(wrapper, "alice"));
      expect(text).toContain("2080 is already used by peer-one (" + NET_A_NAME + ")");
      expect(localField(rowOf(wrapper, "alice")).element.value).toBe("2081");
    });

    it("a free number has no explanation", async () => {
      const { wrapper } = await setup({ b: { rows: [netRow(ALICE, "alice", 80)] } });
      expect(flat(rowOf(wrapper, "alice"))).not.toContain("is already used");
      expect(localField(rowOf(wrapper, "alice")).element.value).toBe("2080");
    });
  });

  describe("a failed open", () => {
    it("a number the node could not bind is told at its row, without the common status line", async () => {
      const busy = { local: 11000, state: "bind_failed", error: "127.0.0.1:11000 cannot be bound" };
      const { wrapper } = await setup({ b: { rows: [netRow(ALICE, "alice", 9000, busy)], opened: [{ node: ALICE, port: 9000, local: 11000 }] } });
      const text = flat(rowOf(wrapper, "alice"));
      expect(text).toContain("port 11000 is busy — choose another number");
      expect(openButton(rowOf(wrapper, "alice")).exists(), "\"Open\" stays for another number").toBe(true);
      expect(wrapper.get("#ports-status").text()).toBe("");
    });

    it("a refusal of the second network's config is told at that row and does not touch the rows of the first network", async () => {
      const { wrapper, node } = await setup({
        a: { rows: [netRow(PEER_ONE, "peer-one", 80)] },
        b: { rows: [netRow(ALICE, "alice", 9000)] },
      });
      node.fail = { path: PUT_B, status: 409, body: { error: "conflict", message: "the node refused to open the port" } };
      await openButton(rowOf(wrapper, "alice")).trigger("click");
      await settle();
      expect(flat(rowOf(wrapper, "alice"))).toContain("the node refused to open the port");
      expect(flat(rowOf(wrapper, "peer-one"))).not.toContain("refused");
      expect(putPaths(node)).toEqual([PUT_B]);
    });
  });
});

describe("one network", () => {
  it("\"Shared with\" keeps the former text and the form keeps the radio choice", async () => {
    const { wrapper } = await setupOne();
    const row = wrapper.get("#ports-own-tbody tr");
    expect(flat(row)).toContain("all members");
    expect(flat(row)).not.toContain(NET_A_NAME + ":");
    expect(wrapper.find("#port-add-audience-all").exists()).toBe(true);
    expect(wrapper.find("#port-add-audience-nodes").exists()).toBe(true);
    expect(flat(wrapper.get("#port-add-audience-all").element.closest("label"))).toBe("All members");
    expect(flat(wrapper.get("#port-add-audience-nodes").element.closest("label"))).toBe("Selected members");
  });

  it("publishing sends exactly one PUT /api/config and nothing to a network address", async () => {
    const { wrapper, node } = await setupOne();
    await wrapper.get("#port-add-port").setValue("9090");
    await wrapper.get("#port-add-btn").trigger("click");
    await settle();
    expect(putPaths(node)).toEqual([PUT_A]);
    expect(node.calls.filter((c) => c.path.startsWith("/api/net/"))).toEqual([]);
  });

  it("a number used by another opened port is not explained under the button", async () => {
    const { wrapper } = await setupOne({ rows: [netRow(PEER_ONE, "peer-one", 80, listening(2080)), netRow(ALICE, "alice", 80)], opened: [{ node: PEER_ONE, port: 80, local: 2080 }] });
    expect(flat(rowOf(wrapper, "alice"))).not.toContain("is already used");
    expect(wrapper.get("#ports-net-tbody").text()).not.toContain("(");
  });
});

// Setup and helpers.

const listening = (local) => ({ local, state: "listening", error: null });
const own = (port, name, audience) => ({ port, name, for: audience });
const netRow = (node, nodeName, port, opened = null) => ({ node, node_name: nodeName, online: true, port, name: "svc" + port, published: true, opened });
const flat = (wrapper) => (wrapper.text ? wrapper.text() : wrapper.textContent).replace(/\s+/g, " ").trim();
const both8080 = () => ({ a: { own: [own(8080, "web", "all")] }, b: { own: [own(8080, "web", "all")] } });

/** The first network holds `local` as an opened port of peer-one; `rest` is merged over the sides. */
function taken(local, rest) {
  const a = { rows: [netRow(PEER_ONE, "peer-one", 80, listening(local))], opened: [{ node: PEER_ONE, port: 80, local }] };
  return { a, b: rest.b };
}

/** One side of the fake node (`own`: published ports, `rows`: network ports, `opened`: the config's opened list, `peers`). */
function applySide(state, config, { own: published = [], rows = [], opened = [], peers = null }) {
  state.ports = { published: published.map(({ port, name }) => ({ port, name, local: "ok" })), network: rows };
  config.ports = { published: published.map((p) => ({ port: p.port, name: p.name, for: p.for })), opened };
  if (peers) state.peers = peers;
}

const peersOfB = () => [
  { self: true, online: true, id: "aaaa1111", id_full: SELF_FULL, name: "finch-unwary", country: "RU" },
  { id: "a11ce000", id_full: ALICE, name: "alice", country: "DE", online: true },
  { id: "b0b00000", id_full: BOB, name: "bob", country: "FR", online: false },
];

/** The page on a node with two networks, the "Ports" tab open and the slot configs loaded. */
async function setup({ a = {}, b = {} } = {}) {
  const opts = twoNets();
  applySide(opts.state, opts.config, a);
  applySide(opts.slots[1].state, opts.slots[1].config, { peers: peersOfB(), ...b });
  const node = createFakeNode(opts);
  const { wrapper, store } = await mountPage(node);
  await openTab(wrapper, "ports");
  await polls(node, 1);
  return { wrapper, node, store };
}

/** The page on a node with one network that publishes 8080 to all. */
async function setupOne({ rows = [], opened = [] } = {}) {
  const state = demoState();
  const config = demoConfig();
  applySide(state, config, { own: [own(8080, "web", "all")], rows, opened });
  const node = createFakeNode({ state, config });
  const { wrapper } = await mountPage(node);
  await openTab(wrapper, "ports");
  await polls(node, 1);
  return { wrapper, node };
}

const puts = (node) => node.calls.filter((c) => c.method === "PUT");
const putPaths = (node) => puts(node).map((c) => c.path);
const lastBody = (node, path) => puts(node).filter((c) => c.path === path).at(-1).body;
const publishedOf = (node, path) => lastBody(node, path).config.ports.published;
const openedOf = (node, path) => lastBody(node, path).config.ports.opened;

const rowOf = (wrapper, text) => wrapper.findAll("#ports-net-tbody tr").find((r) => r.text().includes(text));
const localField = (row) => row.get("input[type=number]");
const openButton = (row) => row.findAll("button").find((b) => b.text() === "Open on this computer") ?? { exists: () => false };

const tick = (input, on) => new DOMWrapper(input).setValue(on);
const boxesOf = (scope) => [...scope.querySelectorAll("input[type=checkbox]")];

/** The nearest ancestor of `el` that holds checkboxes: the publish form or the editor. */
function scopeOf(el) {
  let cur = el.parentElement;
  while (cur && boxesOf(cur).length === 0) cur = cur.parentElement;
  return cur;
}

/** The container of the group labelled "<net>:" inside `scope`. */
function groupOf(scope, net) {
  const marker = [...scope.querySelectorAll("*")].find((e) => e.children.length === 0 && e.textContent.trim() === net + ":");
  if (!marker) throw new Error("no group labelled \"" + net + ":\"");
  return scopeOf(marker);
}

const groupText = (scope, net) => flat(groupOf(scope, net));

function boxIn(scope, net, text) {
  const label = [...groupOf(scope, net).querySelectorAll("label")].find((l) => l.textContent.includes(text));
  if (!label) throw new Error("no box \"" + text + "\" in the group \"" + net + ":\"");
  return label.querySelector("input[type=checkbox]");
}

/** Fills the publish form: `ticks` maps a network name to the box captions to tick there, every other box is cleared first. */
async function when_publishing(wrapper, port, name, ticks) {
  await wrapper.get("#port-add-port").setValue(String(port));
  await wrapper.get("#port-add-name").setValue(name);
  const scope = scopeOf(wrapper.get("#port-add-btn").element);
  for (const box of boxesOf(scope)) await tick(box, false);
  for (const [net, captions] of Object.entries(ticks)) {
    for (const caption of captions) await tick(boxIn(scope, net, caption), true);
  }
  await wrapper.get("#port-add-btn").trigger("click");
  await settle();
}

/** Opens the editor of `port`, lets `change(editorElement)` adjust it and presses "Save". */
async function when_editing(wrapper, port, change) {
  await wrapper.get('[data-port-edit="' + port + '"]').trigger("click");
  await settle();
  await change(scopeOf(wrapper.get("[data-port-edit-save]").element));
  await wrapper.get("[data-port-edit-save]").trigger("click");
  await settle();
}
