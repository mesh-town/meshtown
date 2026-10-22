/* The promise after joining when the new start died, on the whole page (a stub node, the real poller and
 * vitest timers). A visible tab: the node gave `boot-2` + `stopping` and went silent, at the deadline a refusal with the log
 * file. A hidden tab (polling every 10 s, `absence` is still null at the deadline): the snapshot is stuck, a refusal about the "Log".
 * Control: `boot-2` + `offline` on a hidden tab — the promise is dropped. */

import { afterEach, describe, expect, it, vi } from "vitest";
import { demoConfig, demoState } from "../testing/demo-node.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { mountPage, openTab, settle, unmountPage } from "../testing/page.mjs";
import { shown } from "../testing/dom.mjs";

const INVITE = "mesh-invite2-Zq9xK2mP7vLw4TnB8cRd5YfH3jUe6GaS";
const JOIN_PROMISE_MS = 20000;
const JOIN_STALLED = "Something went wrong: Mesh Town didn't respond after joining the network. What happened is on the \"Log\" tab.";
const JOIN_STALLED_SILENT = "Something went wrong: Mesh Town didn't respond after joining the network. What happened is in the meshtown.log file next to the settings file or in the terminal window you started Mesh Town from.";

afterEach(() => {
  unmountPage();
  delete document.hidden;
});

describe("the new start died after joining, the tab is visible", () => {
  it("boot-2 + stopping, then the node is unavailable — #join-status with the log file, visible", async () => {
    const node = makeNode();
    const page = await joined(node);

    node.state = restarted(node.state, "stopping");
    await page.waitFor(4000);
    node.fail = "down";
    await page.waitDeadline();

    expect(page.statusText()).toBe(JOIN_STALLED_SILENT);
    expect(page.statusShown()).toBe(true);
  });
});

describe("the new start died after joining, the tab is hidden", () => {
  it("the boot-2 + stopping snapshot arrived on the poll at +10 s, the node went silent — a refusal about the \"Log\"", async () => {
    hideTab();
    const node = makeNode();
    const page = await joined(node);

    node.state = restarted(node.state, "stopping");
    await page.waitFor(10000);
    node.fail = "down";
    await page.waitDeadline();

    expect(page.statusText()).toBe(JOIN_STALLED);
    expect(page.statusShown()).toBe(true);
  });

  it("control: boot-2 + offline on a hidden tab — the promise is dropped", async () => {
    hideTab();
    const node = makeNode();
    const page = await joined(node);

    node.state = restarted(node.state, "offline");
    await page.waitDeadline();

    expect(page.statusText()).toBe("");
    expect(page.statusShown()).toBe(false);
  });
});

const hideTab = () => Object.defineProperty(document, "hidden", { configurable: true, get: () => true });

const makeNode = () => createFakeNode({
  state: demoState(),
  config: demoConfig(),
  routes: { "POST /api/network/join": { label: "demo-network" } },
});

const restarted = (state, status) => ({ ...state, boot_id: "boot-2", node: { ...state.node, status } });

// The page on the "Settings" tab after clicking "Join": the join is accepted, the deadline has not come yet.
async function joined(node) {
  const { wrapper } = await mountPage(node);
  await openTab(wrapper, "settings");
  await wrapper.get("#join-key-input").setValue(INVITE);
  await wrapper.get("#join-btn").trigger("click");
  await settle();
  const status = () => wrapper.get("#join-status");
  expect(status().text()).toMatch(/^Done: Mesh Town is restarting/);
  return {
    statusText: () => status().text(),
    statusShown: () => shown(status().element),
    waitDeadline: () => advance(JOIN_PROMISE_MS),
    waitFor: advance,
  };
}

async function advance(ms) {
  await vi.advanceTimersByTimeAsync(ms);
  await settle();
}
