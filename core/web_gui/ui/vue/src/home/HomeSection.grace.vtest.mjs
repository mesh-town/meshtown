/* Network status on "Home" and the loading grace period: there is no address directory, but the node has been running for a long time
 * (`node.uptime_sec` ≥ 60 s) — "Home" speaks from what the node knows ("invite" / "all offline"), not "Loading…";
 * with `uptime_sec` < 60 s it is still "Loading network data…". The whole page on a stub node. */

import { afterEach, describe, expect, it } from "vitest";
import { noCatalogOthersText } from "../common/words.ts";
import { DEMO_PEER_ONE, DEMO_PEER_TWO, demoConfig, demoState } from "../testing/demo-node.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { mountPage, unmountPage } from "../testing/page.mjs";
import { shown } from "../testing/dom.mjs";

const LOADING = "Loading network data…";
const OFFLINE_ONE = "The other member is offline — looking for them through the address directory.";
const OFFLINE_ALL = "All other members are offline — looking for them through the address directory.";

afterEach(unmountPage);

describe("the node has been running for a long time (3600 s), no address directory", () => {
  const cases = [
    { tag: "no transport in the snapshot", transport: undefined, one: OFFLINE_ONE, all: OFFLINE_ALL },
    // The grace period ran out and the node itself said "not connected" — "directory unavailable".
    { tag: "directory not connected (joined: false)", transport: dht(false), one: noCatalogOthersText(1), all: noCatalogOthersText(2) },
  ];

  for (const c of cases) {
    describe(c.tag, () => {
      it("on the network only you — the network status on \"Home\" is hidden (the counter already invites)", async () => {
        const { wrapper } = await setup({ peers: selfAndOffline(0), transport: c.transport, uptime: 3600 });

        expect(shown(wrapper.get("#home-net-status").element)).toBe(false);
        expect(statusText(wrapper)).toBe("");
      });

      it("one other member known, offline — \"The other member is offline…\"", async () => {
        const { wrapper } = await setup({ peers: selfAndOffline(1), transport: c.transport, uptime: 3600 });

        expect(statusText(wrapper)).toBe(c.one);
      });

      it("two other members known, offline — \"All other members are offline…\"", async () => {
        const { wrapper } = await setup({ peers: selfAndOffline(2), transport: c.transport, uptime: 3600 });

        expect(statusText(wrapper)).toBe(c.all);
      });
    });
  }

  it("two others offline — there are no words about \"Loading\" on \"Home\"", async () => {
    const { wrapper } = await setup({ peers: selfAndOffline(2), uptime: 3600 });

    expect(statusText(wrapper)).not.toContain("Loading");
  });

  it("another online — the status on \"Home\" is hidden", async () => {
    const { wrapper } = await setup({ peers: demoState().peers, uptime: 3600 });

    expect(shown(wrapper.get("#home-net-status").element)).toBe(false);
  });
});

describe("the 60 s boundary, no address directory", () => {
  it("59 s — still \"Loading network data…\"", async () => {
    const { wrapper } = await setup({ peers: selfAndOffline(2), uptime: 59 });

    expect(statusText(wrapper)).toBe(LOADING);
  });

  it("60 s — already \"All other members are offline…\"", async () => {
    const { wrapper } = await setup({ peers: selfAndOffline(2), uptime: 60 });

    expect(statusText(wrapper)).toBe(OFFLINE_ALL);
  });
});

describe("the node started recently (10 s), no address directory", () => {
  it("on the network only you — no \"Loading network data…\": the status is hidden", async () => {
    const { wrapper } = await setup({ peers: selfAndOffline(0), uptime: 10 });

    expect(shown(wrapper.get("#home-net-status").element)).toBe(false);
    expect(statusText(wrapper)).not.toContain("Loading");
  });

  it("two others known, offline — \"Loading network data…\"", async () => {
    const { wrapper } = await setup({ peers: selfAndOffline(2), uptime: 10 });

    expect(statusText(wrapper)).toBe(LOADING);
  });

  it("uptime unknown (the snapshot has no uptime_sec) — \"Loading network data…\"", async () => {
    const { wrapper } = await setup({ peers: selfAndOffline(2) });

    expect(statusText(wrapper)).toBe(LOADING);
  });
});

describe("the address directory is connected, the node started recently (10 s)", () => {
  it("two others known, offline — \"All other members are offline…\": loading does not hold", async () => {
    const { wrapper } = await setup({ peers: selfAndOffline(2), transport: dht(true), uptime: 10 });

    expect(statusText(wrapper)).toBe(OFFLINE_ALL);
  });

  it("on the network only you — the status is hidden, no \"Click \"Invite a member\"…\" line", async () => {
    const { wrapper } = await setup({ peers: selfAndOffline(0), transport: dht(true), uptime: 10 });

    expect(shown(wrapper.get("#home-net-status").element)).toBe(false);
    expect(statusText(wrapper)).toBe("");
  });
});

const dht = (joined, nodes = 0) => ({ dht: { joined, nodes, last_publish_sec: null } });
const statusText = (wrapper) => wrapper.get("#home-net-status").text().trim();

// Own row and `n` known others, none of them online (of the two demo members).
function selfAndOffline(n) {
  const self = demoState().peers.filter((p) => p.self);
  return [...self, ...[DEMO_PEER_ONE, DEMO_PEER_TWO].slice(0, n).map((p) => ({ ...p, online: false }))];
}

// A page on a node with the given members, the directory and the uptime; `uptime` not set — the snapshot has no `uptime_sec`.
function setup({ peers, transport, uptime }) {
  const base = demoState().node;
  const node = uptime === undefined ? base : { ...base, uptime_sec: uptime };
  return mountPage(createFakeNode({ state: demoState({ peers, transport, node }), config: demoConfig() }));
}
