/* The network status on "Home" on a stub node (moved from e2e: home-status 37, 38, 39): offline peers and DHT
 * are set by the snapshot data (for DemoNode these are the flags `--offline-peers`, `--dht-joined`, `--dht-nodes`).
 * Three states: loading network data (directory not connected), "offline" (others exist, directory connected),
 * a call to invite (no others, directory connected).
 * Visibility is checked by the `hidden`/`style` attribute: happy-dom doesn't compute styles. The tail of the status line
 * "online …" (`#status-line`) is not asserted — it is changed by the branch `wt/ux-join-count`. */

import { afterEach, describe, expect, it } from "vitest";
import { DEMO_PEER_ONE, demoConfig, demoState } from "../testing/demo-node.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { mountPage, settle, unmountPage } from "../testing/page.mjs";
import { shown } from "../testing/dom.mjs";

// Three network states on "Home": loading, "offline", a call to invite.
const LOADING = "Loading network data…";
const OFFLINE_ALL = "All other members are offline — looking for them through the address directory.";
const OFFLINE_ONE = "The other member is offline — looking for them through the address directory.";
const OLD_WORDS = ["Looking for members", "connecting", "Other members so far", "searching via"];
const RAW_NODES = "5 node";

afterEach(unmountPage);

// Like DemoNode's `--offline-peers n`: the last n of the two demo peers are offline.
function offlinePeers(n) {
  const peers = demoState().peers;
  const others = peers.filter((p) => !p.self);
  const offlineFrom = others.length - Math.min(n, others.length);
  const offline = new Set(others.slice(offlineFrom).map((p) => p.id));
  return peers.map((p) => (offline.has(p.id) ? { ...p, online: false, path: undefined, rtt_ms: undefined } : p));
}

const dht = (joined, nodes = 0) => ({ dht: { joined, nodes, last_publish_sec: null } });
const nodeOf = (over) => createFakeNode({ state: demoState(over), config: demoConfig() });
const noNetwork = () => nodeOf({ network: null, peers: [], domains: [], ports: { published: [], network: [] } });

// The reference for the text "you and N others, <tail>": the others are the rows without `self`; the tail counts only them.
function membersText(peers) {
  const others = peers.filter((p) => !p.self);
  const total = others.length;
  const online = others.filter((p) => p.online).length;
  const all = total === 1 ? "online" : total === 2 ? "both online" : "all online";
  const tail = online === total ? all : online === 0 ? "none online" : online + " online";
  return "you and " + total + " " + (total === 1 ? "other" : "others") + ", " + tail;
}

describe("there were members, but all are offline", () => {
  it("the join card is not on Home; Settings links to the add-network tab; \"Invite a member\" is visible", async () => {
    const { wrapper } = await mountPage(nodeOf({ peers: offlinePeers(2) }));

    expect(wrapper.get("#net-join-slot").findAll("a[href='#peers/add']")).toHaveLength(1);
    expect(wrapper.get("#sec-home").findAll("#join-card")).toHaveLength(0);
    const invite = wrapper.get("#netkey-invite-btn");
    expect(shown(invite.element)).toBe(true);
  });
});

describe("the member counter", () => {
  it("home members count (--offline-peers 1): \"you and N others, M online\" from peers, without your own row", async () => {
    const state = demoState({ peers: offlinePeers(1) });
    const { wrapper } = await mountPage(createFakeNode({ state, config: demoConfig() }));

    const count = wrapper.get("#home-members-count");
    expect(shown(count.element)).toBe(true);
    expect(count.text()).toContain(membersText(state.peers));
  });

  it("home members count (--no-network): #home-members-count is hidden and there is no invite card", async () => {
    const { wrapper } = await mountPage(noNetwork());

    expect(wrapper.findAll("#home-members-count")).toHaveLength(0);
  });
});

describe("the network status when no other member is online", () => {
  const cases = [
    { tag: "dht-connecting", transport: dht(false), expected: LOADING, forbidden: OLD_WORDS },
    { tag: "dht-empty", transport: dht(true, 0), expected: OFFLINE_ALL, forbidden: OLD_WORDS },
    { tag: "dht-nodes-seen", transport: dht(true, 5), expected: OFFLINE_ALL, forbidden: [RAW_NODES, ...OLD_WORDS] },
  ];

  for (const c of cases) {
    it(`${c.tag}: two known members offline — the status on Home is visible with the right text`, async () => {
      const { wrapper } = await mountPage(nodeOf({ peers: offlinePeers(2), transport: c.transport }));

      const status = wrapper.get("#home-net-status");
      expect(shown(status.element)).toBe(true);
      expect(status.text()).toContain(c.expected);
      for (const text of c.forbidden) expect(status.text()).not.toContain(text);
    });
  }

  it("one known member offline, directory connected — \"The other member is offline…\"", async () => {
    const peers = [...demoState().peers.filter((p) => p.self), { ...DEMO_PEER_ONE, online: false }];
    const { wrapper } = await mountPage(nodeOf({ peers, transport: dht(true, 0) }));

    const status = wrapper.get("#home-net-status");
    expect(shown(status.element)).toBe(true);
    expect(status.text()).toContain(OFFLINE_ONE);
  });

  it("no transport in the snapshot — directory not connected: \"Loading network data…\"", async () => {
    const { wrapper } = await mountPage(nodeOf({ peers: offlinePeers(2) }));

    expect(wrapper.get("#home-net-status").text()).toContain(LOADING);
  });

  for (const transport of [dht(false), dht(true, 0)]) {
    it(`hidden when another member is online (directory joined: ${transport.dht.joined})`, async () => {
      const { wrapper } = await mountPage(nodeOf({ transport }));

      const status = wrapper.get("#home-net-status");
      expect(shown(status.element)).toBe(false);
    });
  }
});

describe("the network creator is alone (the snapshot has only their own row)", () => {
  const ALONE = "Only you in \"demo-network\" so far. Invite the people you trust.";
  const onlySelf = () => demoState().peers.filter((p) => p.self);
  const cases = [
    { tag: "dht-empty", peers: onlySelf(), transport: dht(true, 0) },
    { tag: "dht-nodes-seen", peers: onlySelf(), transport: dht(true, 5) },
    { tag: "peers-empty", peers: [], transport: dht(true, 0) },
  ];

  for (const c of cases) {
    it(`${c.tag}: the network status is hidden — nothing about \"others\", \"Loading…\" or a second call to invite`, async () => {
      const { wrapper } = await mountPage(nodeOf({ peers: c.peers, transport: c.transport }));

      const status = wrapper.get("#home-net-status");
      expect(shown(status.element)).toBe(false);
      expect(status.text()).toBe("");
    });

    it(`${c.tag}: the "Home" counter — \"Only you in the network so far. Invite the people you trust.\"`, async () => {
      const { wrapper } = await mountPage(nodeOf({ peers: c.peers, transport: c.transport }));

      const count = wrapper.get("#home-members-count");
      expect(shown(count.element)).toBe(true);
      expect(count.text()).toBe(ALONE);
    });
  }

  it("a known offline member appeared — \"All other members are offline…\", without the invite hint", async () => {
    const { wrapper } = await mountPage(nodeOf({ peers: offlinePeers(2), transport: dht(true, 0) }));

    const status = wrapper.get("#home-net-status");
    expect(status.text()).toContain(OFFLINE_ALL);
    expect(status.text()).not.toContain("Invite a member");
  });
});

describe("loading network data (the address directory is not connected)", () => {
  const onlySelf = () => demoState().peers.filter((p) => p.self);
  const selfAndOffline = () => [...onlySelf(), { ...DEMO_PEER_ONE, online: false }];

  it("only your own row — no \"Loading network data…\" in the card: the status is hidden", async () => {
    const { wrapper } = await mountPage(nodeOf({ peers: onlySelf(), transport: dht(false) }));

    const status = wrapper.get("#home-net-status");
    expect(shown(status.element)).toBe(false);
    expect(status.text()).not.toContain(LOADING);
  });

  it("only your own row — the counter says \"Only you in …\", it is the single call to action", async () => {
    const { wrapper } = await mountPage(nodeOf({ peers: onlySelf(), transport: dht(false) }));

    expect(wrapper.get("#home-members-count").text()).toBe("Only you in \"demo-network\" so far. Invite the people you trust.");
  });

  it("your own row and an offline member — also \"Loading network data…\", not \"offline\"", async () => {
    const { wrapper } = await mountPage(nodeOf({ peers: selfAndOffline(), transport: dht(false) }));

    const status = wrapper.get("#home-net-status");
    expect(shown(status.element)).toBe(true);
    expect(status.text()).toContain(LOADING);
    expect(status.text()).not.toContain("offline");
  });

  it("your own row and an offline member — the counter is \"you and 1 other, none online\", not \"Only you…\"", async () => {
    const { wrapper } = await mountPage(nodeOf({ peers: selfAndOffline(), transport: dht(false) }));

    const count = wrapper.get("#home-members-count").text();
    expect(count).toContain("you and 1 other, none online");
    expect(count).not.toContain("Only you");
  });

  it("no transport in the snapshot, only your own row — the status is hidden too", async () => {
    const { wrapper } = await mountPage(nodeOf({ peers: onlySelf() }));

    expect(shown(wrapper.get("#home-net-status").element)).toBe(false);
  });
});

describe("the \"how sites open right now\" panel when the node is silent (store.absence)", () => {
  const HINT = "Start it: the Mesh Town icon near the clock → \"Start Mesh Town\".";

  it("the snapshot is \"stopping\", the node responds (absence null) — \"Mesh Town is stopping…\"", async () => {
    const { wrapper } = await mountPage(stoppingNode());

    expect(routeState(wrapper)).toBe("Mesh Town is stopping…");
  });

  it("the snapshot is \"stopping\", absence \"silent\" — \"Mesh Town is not responding\", not \"is stopping…\"", async () => {
    const { wrapper, store } = await mountPage(stoppingNode());

    await setAbsence(store, "silent");

    expect(routeState(wrapper)).toBe("Mesh Town is not responding");
  });

  it("absence \"silent\" — under the text a hint to start the node with the tray icon", async () => {
    const { wrapper, store } = await mountPage(stoppingNode());

    await setAbsence(store, "silent");

    expect(wrapper.get(PANEL).get(".state-sub").text()).toBe(HINT);
  });

  it("absence \"stopped\" — \"Mesh Town is stopped\"", async () => {
    const { wrapper, store } = await mountPage(stoppingNode());

    await setAbsence(store, "stopped");

    expect(routeState(wrapper)).toBe("Mesh Town is stopped");
  });

  it("absence went back to null — the panel follows the snapshot again: \"Mesh Town is stopping…\"", async () => {
    const { wrapper, store } = await mountPage(stoppingNode());
    await setAbsence(store, "silent");
    expect(routeState(wrapper), "given: while the node is silent, the panel says \"is not responding\"").toBe("Mesh Town is not responding");

    await setAbsence(store, null);

    expect(routeState(wrapper)).toBe("Mesh Town is stopping…");
  });

  it("the exit is offline and the node is silent — there are no \"Choose another\" and \"Open as usual\" buttons", async () => {
    const { wrapper, store } = await mountPage(unavailableNode());
    expect(wrapper.get(PANEL).findAll("button")).toHaveLength(2);

    await setAbsence(store, "silent");

    expect(wrapper.get(PANEL).findAll("button")).toHaveLength(0);
  });
});

const PANEL = '[aria-label="How sites open right now"]';
const routeState = (wrapper) => wrapper.get("#route-state").text();
const stoppingNode = () => nodeOf({ node: { ...demoState().node, status: "stopping" } });
const unavailableNode = () => nodeOf({ route: { state: "unavailable", exits: [{ ...DEMO_PEER_ONE, active: false }] } });

// The store is written by the shell from `conn`; the test sets the reason for silence directly and waits for a redraw.
async function setAbsence(store, absence) {
  store.absence = absence;
  await settle();
}
