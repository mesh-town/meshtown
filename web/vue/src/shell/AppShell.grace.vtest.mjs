/* The header and "Home" and the loading grace period: in one snapshot `#status-line` and `#home-net-status`
 * name the same network state. There is no address directory, the node has been running for a long time (`node.uptime_sec` ≥ 60 s) — the state is
 * from the node's knowledge; the node started recently (< 60 s) — "loading network data…". The whole page on a mock node. */

import { afterEach, describe, expect, it } from "vitest";
import { DEMO_PEER_ONE, DEMO_PEER_TWO, demoConfig, demoState } from "../testing/demo-node.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { mountPage, unmountPage } from "../testing/page.mjs";
import { shown } from "../testing/dom.mjs";

afterEach(unmountPage);

describe("the node has been running for a long time (3600 s), no address directory", () => {
  describe("two others offline", () => {
    it("the header \"all other members are offline\", \"Home\" \"All other members are offline…\"", async () => {
      const { wrapper } = await setup({ peers: selfAndOffline(2), uptime: 3600 });

      expect(textOf(wrapper, "#status-line")).toMatch(/ · all other members are offline$/);
      expect(textOf(wrapper, "#home-net-status")).toMatch(/^All other members are offline/);
    });

    it("the words \"loading network data\" are in neither the header nor \"Home\"", async () => {
      const { wrapper } = await setup({ peers: selfAndOffline(2), uptime: 3600 });

      expect(textOf(wrapper, "#status-line").toLowerCase()).not.toContain("loading network data");
      expect(textOf(wrapper, "#home-net-status").toLowerCase()).not.toContain("loading network data");
    });
  });

  it("one other offline — the header \"the other member is offline\", \"Home\" \"The other member is offline…\"", async () => {
    const { wrapper } = await setup({ peers: selfAndOffline(1), uptime: 3600 });

    expect(textOf(wrapper, "#status-line")).toMatch(/ · the other member is offline$/);
    expect(textOf(wrapper, "#home-net-status")).toMatch(/^The other member is offline/);
  });

  it("only you in the network — the header \"invite members\", the status on \"Home\" is hidden (the counter invites)", async () => {
    const { wrapper } = await setup({ peers: selfAndOffline(0), uptime: 3600 });

    expect(textOf(wrapper, "#status-line")).toMatch(/ · invite members$/);
    expect(shown(wrapper.get("#home-net-status").element)).toBe(false);
  });

  it("others are online — the header \"2 members online\", the status on \"Home\" is hidden", async () => {
    const { wrapper } = await setup({ peers: demoState().peers, uptime: 3600 });

    expect(textOf(wrapper, "#status-line")).toMatch(/ · 2 members online$/);
    expect(shown(wrapper.get("#home-net-status").element)).toBe(false);
  });
});

describe("the node started recently (10 s), no address directory", () => {
  it("two others offline — the header \"loading network data…\", \"Home\" \"Loading network data…\"", async () => {
    const { wrapper } = await setup({ peers: selfAndOffline(2), uptime: 10 });

    expect(textOf(wrapper, "#status-line")).toMatch(/ · loading network data…$/);
    expect(textOf(wrapper, "#home-net-status")).toBe("Loading network data…");
  });

  it("only you in the network — \"loading network data…\" in the header only, the status on \"Home\" is hidden", async () => {
    const { wrapper } = await setup({ peers: selfAndOffline(0), uptime: 10 });

    expect(textOf(wrapper, "#status-line")).toMatch(/ · loading network data…$/);
    expect(shown(wrapper.get("#home-net-status").element)).toBe(false);
  });
});

describe("the 60 s boundary, no address directory, two others offline", () => {
  it("59 s — the header is still \"loading network data…\"", async () => {
    const { wrapper } = await setup({ peers: selfAndOffline(2), uptime: 59 });

    expect(textOf(wrapper, "#status-line")).toMatch(/ · loading network data…$/);
  });

  it("60 s — the header is already \"all other members are offline\"", async () => {
    const { wrapper } = await setup({ peers: selfAndOffline(2), uptime: 60 });

    expect(textOf(wrapper, "#status-line")).toMatch(/ · all other members are offline$/);
  });
});

describe("the address directory is connected, the node started recently (10 s)", () => {
  it("two others offline — loading does not hold: the header and \"Home\" say \"all other members are offline\"", async () => {
    const { wrapper } = await setup({ peers: selfAndOffline(2), transport: dht(true), uptime: 10 });

    expect(textOf(wrapper, "#status-line")).toMatch(/ · all other members are offline$/);
    expect(textOf(wrapper, "#home-net-status")).toMatch(/^All other members are offline/);
  });
});

// The e2e check of the home status line: the header on the first screen — the start and the tail in one line.
describe("the header on the first screen", () => {
  it("\"Sites open …\" at the start, the network label, and \"· 2 members online\" at the end", async () => {
    const { wrapper } = await setup({ peers: demoState().peers, transport: dht(true), uptime: 3600 });

    const line = textOf(wrapper, "#status-line");
    expect(line).toMatch(/^Sites open /);
    expect(line).toMatch(/ · demo-network · 2 members online$/);
    expect(line).not.toContain("In the browser");
  });
});

const dht = (joined, nodes = 0) => ({ dht: { joined, nodes, last_publish_sec: null } });
const textOf = (wrapper, selector) => wrapper.get(selector).text().trim();

// Its own row and `n` known others, none of them online (from two demo members).
function selfAndOffline(n) {
  const self = demoState().peers.filter((p) => p.self);
  return [...self, ...[DEMO_PEER_ONE, DEMO_PEER_TWO].slice(0, n).map((p) => ({ ...p, online: false }))];
}

// The page on a node with the given members, directory and uptime (`uptime_sec` in the snapshot's `node`).
function setup({ peers, transport, uptime }) {
  const node = { ...demoState().node, uptime_sec: uptime };
  return mountPage(createFakeNode({ state: demoState({ peers, transport, node }), config: demoConfig() }));
}
