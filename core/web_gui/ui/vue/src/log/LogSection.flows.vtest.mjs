/* End-to-end cases of the Log tab without a binary or a browser (moved from e2e: log 54, runtime 6): the whole page
 * on a fake node, the feed is served by `GET /api/log` of fake-node. */

import { afterEach, describe, expect, it, vi } from "vitest";
import { demoConfig, demoLog, demoState } from "../testing/demo-node.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { mountPage, openTab, settle, unmountPage } from "../testing/page.mjs";
import { shown } from "../testing/dom.mjs";

const LOG_TICKS = 2;
const RING = 50;
const LOG_POLL_MS = 1000;

afterEach(unmountPage);

const lines = (wrapper) => wrapper.findAll("#logwin [role=listitem]");
const pause = (wrapper) => wrapper.get("#log-pause-btn");

async function openLog(node) {
  const { wrapper } = await mountPage(node);
  await openTab(wrapper, "log");
  return wrapper;
}

// Advances time until the node has received `n` more `GET /api/log` requests.
async function logPolls(node, n) {
  const want = node.count("GET", "/api/log") + n;
  for (let i = 0; node.count("GET", "/api/log") < want; i += 1) {
    if (i > n * 5) throw new Error("the /api/log poll did not reach +" + n);
    await vi.advanceTimersByTimeAsync(LOG_POLL_MS);
  }
  await settle();
}

describe("the log element survives polling", () => {
  it("the line and the \"Pause\" button survive two log polls — the same nodes, the click works the first time", async () => {
    const node = createFakeNode({ state: demoState(), config: demoConfig(), log: demoLog() });
    const wrapper = await openLog(node);
    expect(lines(wrapper).length).toBeGreaterThan(0);
    const line = lines(wrapper).at(-1);
    const button = pause(wrapper);
    expect(shown(wrapper.get("#logwin").element)).toBe(true);
    expect(wrapper.get("#logwin").attributes("aria-label")).toBe("Mesh Town log");
    expect(wrapper.findAll("#logwin [role=list]")).toHaveLength(1);

    await logPolls(node, LOG_TICKS);
    expect(line.element.isConnected).toBe(true);
    expect(button.element.isConnected).toBe(true);
    expect(lines(wrapper).at(-1).element).toBe(line.element);
    expect(pause(wrapper).element).toBe(button.element);

    await button.trigger("click");
    expect(pause(wrapper).text()).toBe("Resume");
    await button.trigger("click");
    expect(pause(wrapper).text()).toBe("Pause");
  });
});

describe("the --ring-lines flag", () => {
  it("--ring-lines 50: the Log stops at 50 lines", async () => {
    const state = demoState();
    state.log.ring_lines = RING;
    const node = createFakeNode({ state, config: demoConfig(), log: demoLog(RING + 30) });
    const wrapper = await openLog(node);
    const first = lines(wrapper).length;
    const last1 = lines(wrapper).at(-1).text();
    node.log.push(...demoLog(5).map((l, i) => ({ ...l, seq: RING + 31 + i, text: "new " + i })));
    await logPolls(node, LOG_TICKS);
    const last2 = lines(wrapper).at(-1).text();
    expect([first, lines(wrapper).length]).toEqual([RING, RING]);
    expect(last2).not.toBe(last1);
  });
});
