/* Shared services with several networks: the panel shows the ports of the network it is opened on and nothing of the others
 * ("My published ports", "Ports shared with you"), "Open" skips a local number already taken in any network and writes into
 * the open network's config, errors are shown at the row. One network keeps its look and requests.
 * The whole page runs on a fake node with two networks: A = "friends" (the main slot), B = "my devices". */

import { afterEach, describe, expect, it } from "vitest";
import { demoConfig, demoState } from "../testing/demo-node.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, polls, settle, unmountPage } from "../testing/page.mjs";
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
    it("lists only the ports of the open network", async () => {
      const { wrapper } = await setup(NET_B, { a: { own: [own(8080, "web", "all")] }, b: { own: [own(9090, "db", "all")] } });
      const rows = wrapper.findAll("#ports-own-tbody tr");
      expect(rows).toHaveLength(1);
      expect(flat(rows[0])).toContain("9090");
      expect(flat(rows[0])).not.toContain("8080");
    });

    it("the same port number published in both networks is one row in the panel of the main network", async () => {
      const { wrapper } = await setup("main", { a: { own: [own(8080, "web", "all")] }, b: { own: [own(8080, "web", [ALICE])] } });
      expect(wrapper.findAll("#ports-own-tbody tr")).toHaveLength(1);
      expect(flat(wrapper.get("#ports-own-tbody tr"))).toContain("all members");
    });

    it("the same port number published in both networks is one row in the panel of the second network", async () => {
      const { wrapper } = await setup(NET_B, { a: { own: [own(8080, "web", "all")] }, b: { own: [own(8080, "web", [ALICE])] } });
      expect(wrapper.findAll("#ports-own-tbody tr")).toHaveLength(1);
      expect(flat(wrapper.get("#ports-own-tbody tr"))).toContain("alice");
    });

    it("\"Shared with\" names the members of the open network, without a network prefix", async () => {
      const { wrapper } = await setup(NET_B, { b: { own: [own(8080, "web", [ALICE])] } });
      const row = flat(wrapper.get("#ports-own-tbody tr"));
      expect(row).toContain("alice");
      expect(row).not.toContain(NET_B_NAME + ":");
      expect(row).not.toContain(" · ");
    });

    it("a network without published ports shows no port rows although the other has some", async () => {
      const { wrapper } = await setup(NET_B, { a: { own: [own(8080, "web", "all")] } });
      expect(wrapper.findAll("#ports-own-tbody [data-port-unpublish]")).toHaveLength(0);
    });
  });

  describe("ports shared with you", () => {
    it("only the rows of the open network, the member named without the network", async () => {
      const { wrapper } = await setup(NET_B, {
        a: { rows: [netRow(PEER_ONE, "peer-one", 80)] },
        b: { rows: [netRow(ALICE, "alice", 9000)] },
      });
      const rows = wrapper.findAll("#ports-net-tbody tr");
      expect(rows).toHaveLength(1);
      expect(flat(rows[0])).toContain("alice");
      expect(flat(rows[0])).not.toContain("(" + NET_B_NAME + ")");
      expect(flat(wrapper.get("#ports-net-tbody"))).not.toContain("peer-one");
    });

    it("the same member and port in both networks is one row in the panel of the main network", async () => {
      const both = { a: { rows: [netRow(PEER_ONE, "peer-one", 80)] }, b: { rows: [netRow(PEER_ONE, "peer-one", 80)] } };
      const { wrapper } = await setup("main", both);
      expect(wrapper.findAll("#ports-net-tbody tr")).toHaveLength(1);
      expect(flat(wrapper.get("#ports-net-tbody"))).not.toContain("(" + NET_A_NAME + ")");
    });
  });

  describe("opening a member's port of the second network", () => {
    it("the local number 11000 is taken in the first network, so the field starts with 11001", async () => {
      const { wrapper } = await setup(NET_B, taken(11000, { b: { rows: [netRow(ALICE, "alice", 9000)] } }));
      expect(localField(rowOf(wrapper, "alice")).element.value).toBe("11001");
    });

    it("\"Open\" writes the pair with 11001 into the config of the second network only", async () => {
      const { wrapper, node } = await setup(NET_B, taken(11000, { b: { rows: [netRow(ALICE, "alice", 9000)] } }));
      await openButton(rowOf(wrapper, "alice")).trigger("click");
      await settle();
      expect(putPaths(node)).toEqual([PUT_B]);
      expect(openedOf(node, PUT_B)).toContainEqual({ node: ALICE, port: 9000, local: 11001 });
      expect(openedOf(node, PUT_B)).toHaveLength(1);
    });

    it("a number taken in the first network is explained under the button before the click, naming who uses it", async () => {
      const { wrapper } = await setup(NET_B, taken(2080, { b: { rows: [netRow(ALICE, "alice", 80)] } }));
      const text = flat(rowOf(wrapper, "alice"));
      expect(text).toContain("2080 is already used by peer-one (" + NET_A_NAME + ")");
      expect(localField(rowOf(wrapper, "alice")).element.value).toBe("2081");
    });

    it("a free number has no explanation", async () => {
      const { wrapper } = await setup(NET_B, { b: { rows: [netRow(ALICE, "alice", 80)] } });
      expect(flat(rowOf(wrapper, "alice"))).not.toContain("is already used");
      expect(localField(rowOf(wrapper, "alice")).element.value).toBe("2080");
    });
  });

  describe("a failed open", () => {
    it("a number the node could not bind is told at its row, without the common status line", async () => {
      const busy = { local: 11000, state: "bind_failed", error: "127.0.0.1:11000 cannot be bound" };
      const { wrapper } = await setup(NET_B, { b: { rows: [netRow(ALICE, "alice", 9000, busy)], opened: [{ node: ALICE, port: 9000, local: 11000 }] } });
      const text = flat(rowOf(wrapper, "alice"));
      expect(text).toContain("port 11000 is busy — choose another number");
      expect(openButton(rowOf(wrapper, "alice")).exists(), "\"Open\" stays for another number").toBe(true);
      expect(wrapper.get("#ports-status").text()).toBe("");
    });

    it("a refusal of the network's config is told at that row", async () => {
      const { wrapper, node } = await setup(NET_B, { b: { rows: [netRow(ALICE, "alice", 9000)] } });
      node.fail = { path: PUT_B, status: 409, body: { error: "conflict", message: "the node refused to open the port" } };
      await openButton(rowOf(wrapper, "alice")).trigger("click");
      await settle();
      expect(flat(rowOf(wrapper, "alice"))).toContain("the node refused to open the port");
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

  it("members are named without a network", async () => {
    const { wrapper } = await setupOne({ rows: [netRow(PEER_ONE, "peer-one", 80, listening(2080)), netRow(ALICE, "alice", 80)], opened: [{ node: PEER_ONE, port: 80, local: 2080 }] });
    expect(wrapper.get("#ports-net-tbody").text()).not.toContain("(");
  });
});

// Setup and helpers.

const listening = (local) => ({ local, state: "listening", error: null });
const own = (port, name, audience) => ({ port, name, for: audience });
const netRow = (node, nodeName, port, opened = null) => ({ node, node_name: nodeName, online: true, port, name: "svc" + port, published: true, opened });
const flat = (wrapper) => (wrapper.text ? wrapper.text() : wrapper.textContent).replace(/\s+/g, " ").trim();

/** The first network holds `local` as an opened port of peer-one; `rest.b` holds the second network. */
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

/** The page on a node with two networks, opened on Shared services of `slot`, the slot configs loaded. */
async function setup(slot, { a = {}, b = {} } = {}) {
  const opts = twoNets();
  applySide(opts.state, opts.config, a);
  applySide(opts.slots[1].state, opts.slots[1].config, { peers: peersOfB(), ...b });
  const node = createFakeNode(opts);
  const { wrapper, store } = await mountPage(node, { hash: "peers/" + slot + "/services" });
  await polls(node, 1);
  return { wrapper, node, store };
}

/** The page on a node with one network that publishes 8080 to all. */
async function setupOne({ rows = [], opened = [] } = {}) {
  const state = demoState();
  const config = demoConfig();
  applySide(state, config, { own: [own(8080, "web", "all")], rows, opened });
  const node = createFakeNode({ state, config });
  const { wrapper } = await mountPage(node, { hash: "peers/main/services" });
  await polls(node, 1);
  return { wrapper, node };
}

const puts = (node) => node.calls.filter((c) => c.method === "PUT");
const putPaths = (node) => puts(node).map((c) => c.path);
const lastBody = (node, path) => puts(node).filter((c) => c.path === path).at(-1).body;
const openedOf = (node, path) => lastBody(node, path).config.ports.opened;

const rowOf = (wrapper, text) => wrapper.findAll("#ports-net-tbody tr").find((r) => r.text().includes(text));
const localField = (row) => row.get("input[type=number]");
const openButton = (row) => row.findAll("button").find((b) => b.text() === "Open on this computer") ?? { exists: () => false };
