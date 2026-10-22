/* The header and "Home" with an unavailable address directory: in one snapshot `#status-line` and
 * `#home-net-status` name the same network state. The node has been running for a long time (`node.uptime_sec` ≥ 60 s), itself reported
 * `transport.dht.joined === false`, none of the others are online — "Home" names both "offline" and the unavailable directory; the header says
 * "members can't find you automatically" only when there are no other rows, otherwise "offline". The words are verbatim. The whole page on a mock node. */

import { afterEach, describe, expect, it } from "vitest";
import { DEMO_PEER_ONE, DEMO_PEER_TWO, demoConfig, demoState } from "../testing/demo-node.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { mountPage, unmountPage } from "../testing/page.mjs";
import { shown } from "../testing/dom.mjs";

afterEach(unmountPage);

describe("the node has been running for a long time (120 s), the directory is not connected (joined: false), no others online", () => {
  describe("only you in the network", () => {
    it("the header \"… · members can't find you automatically\", the status on \"Home\" is hidden (the counter invites)", async () => {
      const page = await setup({ peers: selfAndOffline(0) });

      expect(page.header()).toMatch(/ · members can't find you automatically$/);
      expect(page.statusShown()).toBe(false);
      expect(page.status()).toBe("");
    });
  });

  describe("one other offline", () => {
    it("the header \"… · the other member is offline\" (the directory is named by \"Home\")", async () => {
      const page = await setup({ peers: selfAndOffline(1) });

      expect(page.header()).toMatch(/ · the other member is offline$/);
      expect(page.header()).not.toContain("address directory");
    });

    it("\"Home\" \"The other member is offline, and the address directory that helps members find each other is unavailable. Send an invite…\"", async () => {
      const page = await setup({ peers: selfAndOffline(1) });

      expect(page.status()).toMatch(/^The other member is offline, and the address directory that helps members find each other is unavailable. Send an invite/);
    });
  });

  describe("two others offline", () => {
    it("the header \"… · all other members are offline\", \"Home\" \"All other members are offline, and the address directory that helps members find each other is unavailable…\"", async () => {
      const page = await setup({ peers: selfAndOffline(2) });

      expect(page.header()).toMatch(/ · all other members are offline$/);
      expect(page.status()).toMatch(/^All other members are offline, and the address directory that helps members find each other is unavailable/);
    });
  });

  describe("the header and \"Home\" name the same state", () => {
    it("no others — the header names the unavailable directory, \"Home\" has no status, no \"loading\"", async () => {
      const page = await setup({ peers: selfAndOffline(0) });

      expect(page.header()).toContain("members can't find you automatically");
      expect(page.status()).toBe("");
      expect((page.header() + page.status()).toLowerCase()).not.toContain("loading network data");
    });

    for (const n of [1, 2]) {
      it(`others offline: ${n} — the header says only "offline", "Home" says both "offline" and "the address directory that helps members find each other is unavailable"`, async () => {
        const page = await setup({ peers: selfAndOffline(n) });

        expect(page.header()).toContain("offline");
        expect(page.header()).not.toContain("address directory");
        expect(page.status()).toContain("offline");
        expect(page.status().toLowerCase()).toContain("the address directory that helps members find each other is unavailable");
        expect((page.header() + page.status()).toLowerCase()).not.toContain("loading network data");
      });
    }
  });
});

describe("another is online, the directory is not connected (joined: false), the node has been running for a long time", () => {
  it("the header \"… · 2 members online\", the status on \"Home\" is hidden", async () => {
    const page = await setup({ peers: demoState().peers });

    expect(page.header()).toMatch(/ · 2 members online$/);
    expect(page.statusShown()).toBe(false);
  });
});

describe("the node started recently (30 s), the directory is not connected", () => {
  it("the header \"… · loading network data…\", \"Home\" \"Loading network data…\"", async () => {
    const page = await setup({ peers: selfAndOffline(1), uptime: 30 });

    expect(page.header()).toMatch(/ · loading network data…$/);
    expect(page.status()).toBe("Loading network data…");
  });
});

describe("the directory is connected (joined: true), the node has been running for a long time — the old words", () => {
  it("one other offline — the header \"the other member is offline\", \"Home\" \"… — looking for them through the address directory.\"", async () => {
    const page = await setup({ peers: selfAndOffline(1), joined: true });

    expect(page.header()).toMatch(/ · the other member is offline$/);
    expect(page.status()).toBe("The other member is offline — looking for them through the address directory.");
  });

  it("only you in the network — the header \"invite members\", no \"Click \"Invite a member\"…\" status on \"Home\"", async () => {
    const page = await setup({ peers: selfAndOffline(0), joined: true });

    expect(page.header()).toMatch(/ · invite members$/);
    expect(page.statusShown()).toBe(false);
  });
});

// Its own row and `n` known others, none of them online (from two demo members).
function selfAndOffline(n) {
  const self = demoState().peers.filter((p) => p.self);
  return [...self, ...[DEMO_PEER_ONE, DEMO_PEER_TWO].slice(0, n).map((p) => ({ ...p, online: false }))];
}

const dht = (joined) => ({ dht: { joined, nodes: 0, last_publish_sec: null } });

// The page on a node with the given members; the directory is not connected by default, the node has been running for 120 s.
async function setup({ peers, joined = false, uptime = 120 }) {
  const node = { ...demoState().node, uptime_sec: uptime };
  const { wrapper } = await mountPage(createFakeNode({ state: demoState({ peers, transport: dht(joined), node }), config: demoConfig() }));
  return {
    header: () => wrapper.get("#status-line").text().trim(),
    status: () => wrapper.get("#home-net-status").text().trim(),
    statusShown: () => shown(wrapper.get("#home-net-status").element),
  };
}
