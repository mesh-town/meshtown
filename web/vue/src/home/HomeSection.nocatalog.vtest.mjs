/* "Address directory unavailable" on "Home": the node has been running a while (`node.uptime_sec` ≥ 60 s),
 * reported itself `transport.dht.joined === false`, no one else is online — `#home-net-status` says the directory is
 * unavailable and leads to "Invite a member" (an invite with your own address, joining without the directory). There is one button, in the same
 * card `#home-invite` as the status, and it is enabled. The words are verbatim, not imported constants. The whole page is on a stub node. */

import { afterEach, describe, expect, it } from "vitest";
import { DEMO_PEER_ONE, DEMO_PEER_TWO, demoConfig, demoState } from "../testing/demo-node.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { mountPage, unmountPage } from "../testing/page.mjs";
import { shown } from "../testing/dom.mjs";

const NO_CATALOG_ONE =
  "The other member is offline, and the address directory that helps members find each other is unavailable. Send an invite: it contains your address, so they can connect without the directory.";
const NO_CATALOG_ALL =
  "All other members are offline, and the address directory that helps members find each other is unavailable. Send an invite: it contains your address, so they can connect without the directory.";
const OFFLINE_ONE = "The other member is offline — looking for them through the address directory.";
const LOADING = "Loading network data…";

afterEach(unmountPage);

describe("the node has been running a while (120 s), the directory is not connected (joined: false)", () => {
  describe("only you in the network", () => {
    it("the status is hidden: the counter above already says to invite people, and there is nobody to look for", async () => {
      const page = await setup({ peers: selfAndOffline(0) });

      expect(page.statusShown()).toBe(false);
      expect(page.statusText()).toBe("");
    });
  });

  describe("one other member offline", () => {
    it("the status is visible: \"The other member is offline, and the address directory that helps members find each other is unavailable. Send an invite…\"", async () => {
      const page = await setup({ peers: selfAndOffline(1) });

      expect(page.statusShown()).toBe(true);
      expect(page.statusText()).toBe(NO_CATALOG_ONE);
    });
  });

  describe("two other members offline", () => {
    it("the status: \"All other members are offline, and the address directory that helps members find each other is unavailable. Send an invite…\"", async () => {
      const page = await setup({ peers: selfAndOffline(2) });

      expect(page.statusText()).toBe(NO_CATALOG_ALL);
    });
  });

  describe("the \"Invite a member\" button", () => {
    const cases = [
      { tag: "only you in the network", peers: () => selfAndOffline(0) },
      { tag: "one other member offline", peers: () => selfAndOffline(1) },
    ];

    for (const c of cases) {
      describe(c.tag, () => {
        it("exactly one button: we don't draw a second one next to the status", async () => {
          const page = await setup({ peers: c.peers() });

          expect(page.inviteButtons()).toHaveLength(1);
        });

        it("the button is in the same \"Network invite\" card as the network status", async () => {
          const page = await setup({ peers: c.peers() });

          expect(page.inviteCards()).toHaveLength(1);
          expect(page.statusCard()).not.toBeNull();
          expect(page.buttonCard()).toBe(page.statusCard());
        });

        it("the button is not disabled: it can be clicked right away", async () => {
          const page = await setup({ peers: c.peers() });

          expect(page.inviteButton().attributes("disabled")).toBeUndefined();
          expect(shown(page.inviteButton().element)).toBe(true);
        });
      });
    }
  });

  describe("another member online", () => {
    it("the status is hidden: a connection with a member matters more than an unavailable directory", async () => {
      const page = await setup({ peers: demoState().peers });

      expect(page.statusShown()).toBe(false);
    });
  });
});

describe("the node started recently (30 s), the directory is not connected", () => {
  it("\"Loading network data…\", not \"directory unavailable\"", async () => {
    const page = await setup({ peers: selfAndOffline(1), uptime: 30 });

    expect(page.statusText()).toBe(LOADING);
  });

  it("the \"Invite a member\" button is still single and not disabled", async () => {
    const page = await setup({ peers: selfAndOffline(1), uptime: 30 });

    expect(page.inviteButtons()).toHaveLength(1);
    expect(page.inviteButton().attributes("disabled")).toBeUndefined();
  });
});

describe("the node has been running a while (120 s), the directory is connected (joined: true) — the old words", () => {
  it("only you in the network — the status is hidden, no \"Click \"Invite a member\"…\" line", async () => {
    const page = await setup({ peers: selfAndOffline(0), joined: true });

    expect(page.statusShown()).toBe(false);
    expect(page.statusText()).toBe("");
  });

  it("one other member offline — \"The other member is offline — looking for them through the address directory.\"", async () => {
    const page = await setup({ peers: selfAndOffline(1), joined: true });

    expect(page.statusText()).toBe(OFFLINE_ONE);
  });
});

// Your own row and `n` known others, none of them online (out of two demo members).
function selfAndOffline(n) {
  const self = demoState().peers.filter((p) => p.self);
  return [...self, ...[DEMO_PEER_ONE, DEMO_PEER_TWO].slice(0, n).map((p) => ({ ...p, online: false }))];
}

const dht = (joined) => ({ dht: { joined, nodes: 0, last_publish_sec: null } });
const closestInvite = (el) => el.closest("#home-invite");

// The page on a node with the given members; the directory is not connected by default, the node has been running for 120 s.
async function setup({ peers, joined = false, uptime = 120 }) {
  const node = { ...demoState().node, uptime_sec: uptime };
  const { wrapper } = await mountPage(createFakeNode({ state: demoState({ peers, transport: dht(joined), node }), config: demoConfig() }));
  return {
    statusText: () => wrapper.get("#home-net-status").text().trim(),
    statusShown: () => shown(wrapper.get("#home-net-status").element),
    inviteButtons: () => wrapper.findAll("#netkey-invite-btn"),
    inviteButton: () => wrapper.get("#netkey-invite-btn"),
    inviteCards: () => wrapper.findAll("#home-invite"),
    statusCard: () => closestInvite(wrapper.get("#home-net-status").element),
    buttonCard: () => closestInvite(wrapper.get("#netkey-invite-btn").element),
  };
}
