/* "Exit via …" on the "Site checks" tab with two networks (A = `main`, B): the exit goes into the network that carries the browser proxy. */

import { afterEach, describe, expect, it } from "vitest";
import { createFakeNode } from "../testing/fake-node.mjs";
import { mountPage, openTab, settle, unmountPage } from "../testing/page.mjs";
import { NET_B, twoNets } from "../testing/overview-fixtures.mjs";

const SELF_ID = "aaaa1111";
const FULL_A = "B".repeat(64);
const FULL_B = "D".repeat(64);

afterEach(unmountPage);

const exitPeer = (id, idFull, name) => ({ id, id_full: idFull, name, country: null, version: "0.1.0", online: true, path: "direct", rtt_ms: 20, exit: true });
const domain = (peerId) => ({ name: "example.org", checks: { [SELF_ID]: { verdict: "network_blocked", reason: "reset", age_sec: 5 }, [peerId]: { verdict: "ok", age_sec: 5 } } });

// `proxy` — "main", NET_B or null (nobody); `exitsB` — the full ids B already routes through.
async function setup({ proxy = "main", exitsB = [] } = {}) {
  const opts = twoNets();
  const [a, b] = [opts.state, opts.slots[1].state];
  const self = a.peers.find((p) => p.self);
  a.peers = [self, exitPeer("bbbb2222", FULL_A, "laptop")];
  b.peers = [self, exitPeer("dddd4444", FULL_B, "phone")];
  a.domains = [domain("bbbb2222")];
  b.domains = [domain("dddd4444")];
  b.route = { state: exitsB.length ? "exit" : "direct", exits: exitsB.map((id) => ({ ...b.peers[1], id_full: id, active: true })) };
  for (const config of [opts.config, opts.slots[1].config]) config.ports.published = [{ port: 8080, name: "web8080", for: "all" }];
  opts.slots[0].proxy = proxy === "main";
  opts.slots[1].proxy = proxy === NET_B;
  const node = createFakeNode(opts);
  const { wrapper } = await mountPage(node);
  await openTab(wrapper, "domains");
  return { node, wrapper };
}

const puts = (node) => node.calls.filter((c) => c.method === "PUT");
const exitButton = (wrapper) => wrapper.findAll("#domains-tbody button").find((b) => b.text().startsWith("Exit via"));

async function click(wrapper) {
  await exitButton(wrapper).trigger("click");
  await settle();
}

describe("DomainsSection exit button with two networks", () => {
  it("proxy in main: one PUT /api/config with the member", async () => {
    const { node, wrapper } = await setup({ proxy: "main" });
    await click(wrapper);
    expect(puts(node).map((c) => c.path)).toEqual(["/api/config"]);
    expect(puts(node)[0].body.config.route.exits).toEqual([FULL_A]);
  });

  it("proxy in B while A leads: one PUT /api/net/B/config, no PUT /api/config", async () => {
    const { node, wrapper } = await setup({ proxy: NET_B });
    await click(wrapper);
    expect(puts(node).map((c) => c.path)).toEqual(["/api/net/" + NET_B + "/config"]);
    expect(puts(node)[0].body.config.route.exits).toEqual([FULL_B]);
  });

  it("nobody carries the proxy: the first network gets the exit", async () => {
    const { node, wrapper } = await setup({ proxy: null });
    await click(wrapper);
    expect(puts(node).map((c) => c.path)).toEqual(["/api/config"]);
    expect(puts(node)[0].body.config.route.exits).toEqual([FULL_A]);
  });

  it("the exit chosen in B is marked on the Sites row while A leads", async () => {
    const { wrapper } = await setup({ proxy: NET_B, exitsB: [FULL_B] });
    expect(wrapper.get("#domains-tbody").text()).toContain("✓ Your exit");
    expect(exitButton(wrapper)).toBeUndefined();
  });

  it("one network: PUT /api/config as before", async () => {
    const opts = twoNets();
    const self = opts.state.peers.find((p) => p.self);
    opts.state.peers = [self, exitPeer("bbbb2222", FULL_A, "laptop")];
    opts.state.domains = [domain("bbbb2222")];
    opts.config.ports.published = [{ port: 8080, name: "web8080", for: "all" }];
    const node = createFakeNode({ state: opts.state, config: opts.config });
    const { wrapper } = await mountPage(node);
    await openTab(wrapper, "domains");
    await click(wrapper);
    expect(puts(node).map((c) => c.path)).toEqual(["/api/config"]);
  });
});
