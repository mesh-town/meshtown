/* The promise after joining, on the whole page: a stub node, the real `api.js`, poller and vitest timers.
 * The join is accepted — under the button "Done: … is restarting"; after 20 s it is either dropped (the node replied after
 * joining — a new `boot_id` in the snapshot), or replaced by a refusal. Visibility is checked by the `hidden` attribute. */

import { afterEach, describe, expect, it, vi } from "vitest";
import { demoConfig, demoState } from "../testing/demo-node.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { mountPage, openTab, settle, unmountPage } from "../testing/page.mjs";
import { shown } from "../testing/dom.mjs";

const INVITE = "mesh-invite2-Zq9xK2mP7vLw4TnB8cRd5YfH3jUe6GaS";
const JOIN_PROMISE_MS = 20000;
const JOIN_STALLED = "Something went wrong: Mesh Town didn't respond after joining the network. What happened is on the \"Log\" tab.";
const JOIN_STALLED_SILENT = "Something went wrong: Mesh Town didn't respond after joining the network. What happened is in the meshtown.log file next to the settings file or in the terminal window you started Mesh Town from.";

afterEach(unmountPage);

describe("the promise after joining", () => {
  describe("the node replied after joining with a new start, but with no members", () => {
    it("#join-status is empty and hidden: no refusal, although node.status is offline", async () => {
      const node = makeNode();
      const page = await joined(node);
      page.expectPromise();

      node.state = restartedAlone(node.state);
      await page.waitDeadline();

      expect(page.statusText()).toBe("");
      expect(page.statusShown()).toBe(false);
    });
  });

  describe("the node replies with the old start, node.status is online", () => {
    it("#join-status shows the \"didn't respond\" refusal: a stale online doesn't count as a reply", async () => {
      const node = makeNode();
      const page = await joined(node);
      page.expectPromise();

      await page.waitDeadline();

      expect(page.statusText()).toBe(JOIN_STALLED);
      expect(page.statusShown()).toBe(true);
    });
  });

  describe("after joining the node is unavailable (silent: polls fail)", () => {
    it("#join-status shows the \"didn't respond\" refusal with the meshtown.log file instead of the \"Log\" tab", async () => {
      const node = makeNode();
      const page = await joined(node);
      page.expectPromise();

      node.fail = "down";
      await page.waitDeadline();

      expect(page.statusText()).toBe(JOIN_STALLED_SILENT);
      expect(page.statusShown()).toBe(true);
    });
  });

  describe("after joining the node replies 503 (stopped)", () => {
    it("#join-status shows the same refusal with the meshtown.log file, without the \"Log\" tab", async () => {
      const node = makeNode();
      const page = await joined(node);
      page.expectPromise();

      node.fail = { path: "/api/state", status: 503, body: { error: "stopping" } };
      await page.waitDeadline();

      expect(page.statusText()).toBe(JOIN_STALLED_SILENT);
      expect(page.statusShown()).toBe(true);
    });
  });

  describe("the node went silent, but by the deadline replied again with the old start", () => {
    it("#join-status shows the old refusal about the \"Log\" tab: Mesh Town is alive, the feed is readable", async () => {
      const node = makeNode();
      const page = await joined(node);
      page.expectPromise();

      // Two refusals in a row — the "not responding" strip, the next poll in 5 s (backoff); the node comes alive before it.
      node.fail = "down";
      await page.waitFor(5000);
      node.fail = null;
      await page.waitDeadline();

      expect(page.statusText()).toBe(JOIN_STALLED);
    });
  });
});

const makeNode = () => createFakeNode({
  state: demoState(),
  config: demoConfig(),
  routes: { "POST /api/network/join": { label: "demo-network" } },
});

// A restarted node that is alone in the network: a new `boot_id`, no connection (`offline`), only you in `peers`.
const restartedAlone = (state) => ({
  ...state,
  boot_id: "boot-2",
  node: { ...state.node, status: "offline" },
  peers: state.peers.filter((peer) => peer.self),
});

// The page on the "Settings" tab after clicking "Join": the join is accepted, the deadline has not come yet.
async function joined(node) {
  const { wrapper } = await mountPage(node);
  await openTab(wrapper, "settings");
  await wrapper.get("#join-key-input").setValue(INVITE);
  await wrapper.get("#join-btn").trigger("click");
  await settle();
  const status = () => wrapper.get("#join-status");
  return {
    statusText: () => status().text(),
    statusShown: () => shown(status().element),
    expectPromise: () => expect(status().text()).toMatch(/^Done: Mesh Town is restarting/),
    waitDeadline: async () => {
      await vi.advanceTimersByTimeAsync(JOIN_PROMISE_MS);
      await settle();
    },
    // Part of the deadline: the promise is still in place, polling goes on.
    waitFor: async (ms) => {
      await vi.advanceTimersByTimeAsync(ms);
      await settle();
    },
  };
}
