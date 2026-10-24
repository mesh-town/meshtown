/* "Use as exit" on the "Members" tab of a node with two networks: the choice is written into the network of the open tab,
 * and the buttons of that tab follow the exit of its own network. */

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { createFakeNode } from "../testing/fake-node.mjs";
import { memberRow, membersReply, selfRow } from "../testing/members-fixtures.mjs";
import { NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, openTab, settle, unmountPage } from "../testing/page.mjs";
import { baseConfig, baseState } from "../testing/section.mjs";

const REMEMBERED_KEY = "meshtown.exitsRemembered";
const FULL_ONE = "A".repeat(64);
const FULL_TWO = "B".repeat(64);
const SELECTOR = "[data-exit-via]";

beforeEach(() => localStorage.clear());
afterEach(unmountPage);

const exitPeer = (idFull, name) => ({ id: idFull.slice(0, 8).toLowerCase(), id_full: idFull, name, country: "NL", version: "0.1.0", online: true, path: "direct", rtt_ms: 20, exit: true });

function netB(exits = []) {
  const peers = [exitPeer(FULL_ONE, "laptop"), exitPeer(FULL_TWO, "phone")];
  const base = baseState({ network: { id_hex: "net-bbbb", masked: "mesh1-••••-••••" }, peers: [baseState().peers.find((p) => p.self), ...peers] });
  const state = { ...base, route: { state: exits.length ? "exit" : "direct", exits: exits.map((id) => ({ ...peers.find((p) => p.id_full === id), active: true })) } };
  const rows = [selfRow(), ...peers.map((p) => memberRow({ id: p.id, id_full: p.id_full, name: p.name, online: true }))];
  return { state, config: baseConfig({ route: { exits } }), members: membersReply({ rows }) };
}

async function open(hash, exits = []) {
  const opts = twoNets({ ...netB(exits), proxy: exits.length > 0 });
  opts.slots[0].proxy = exits.length === 0;
  const node = createFakeNode(opts);
  const { wrapper } = await mountPage(node, { hash });
  return { node, wrapper };
}

const puts = (node) => node.calls.filter((c) => c.method === "PUT");
const paths = (node) => puts(node).map((c) => c.path);

describe("choosing an exit on the tab of another network", () => {
  it("writes route.exits into that network, not into the primary one", async () => {
    const { node, wrapper } = await open("peers/" + NET_B);
    await wrapper.get(`[data-exit-via="${FULL_ONE}"]`).trigger("click");
    await settle();
    expect(paths(node)).toEqual(["/api/net/" + NET_B + "/config"]);
    expect(puts(node)[0].body.config.route.exits).toEqual([FULL_ONE]);
  });

  it("remembers the exit under the slot of the tab only", async () => {
    const { wrapper } = await open("peers/" + NET_B);
    await wrapper.get(`[data-exit-via="${FULL_ONE}"]`).trigger("click");
    await settle();
    expect(JSON.parse(localStorage.getItem(REMEMBERED_KEY))).toEqual([{ id: FULL_ONE, slot: NET_B }]);
  });

  it("keeps writing the primary config from the tab of the primary network", async () => {
    const { node, wrapper } = await open("peers");
    const button = wrapper.find(SELECTOR);
    expect(button.exists()).toBe(true);
    const idFull = button.attributes("data-exit-via");
    await button.trigger("click");
    await settle();
    expect(paths(node)).toEqual(["/api/config"]);
    expect(JSON.parse(localStorage.getItem(REMEMBERED_KEY))).toEqual([{ id: idFull, slot: "main" }]);
  });

  it("hides the button at the exit that the network of the tab already uses", async () => {
    const { wrapper } = await open("peers/" + NET_B, [FULL_ONE]);
    expect(wrapper.find(`[data-exit-via="${FULL_ONE}"]`).exists()).toBe(false);
    expect(wrapper.find(`[data-exit-via="${FULL_TWO}"]`).exists()).toBe(true);
  });

  it("the main save after Use as exit in B has no stale_rev", async () => {
    const { node, wrapper } = await open("peers/" + NET_B);
    // The node rewrites the primary config while it moves the proxy; the reply to the write into B does not say so.
    node.slotList[1].routes["PUT /api/config"] = (body) => {
      node.rev += 2;
      return { rev: body.rev + 1, config: body.config };
    };
    await wrapper.get(`[data-exit-via="${FULL_ONE}"]`).trigger("click");
    await settle();
    await openTab(wrapper, "peers");
    await wrapper.find(SELECTOR).trigger("click");
    await settle();
    expect(puts(node).filter((c) => c.path === "/api/config"), "one write of the primary config, no retry").toHaveLength(1);
  });
});
