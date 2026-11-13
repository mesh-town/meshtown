/* The exit-owner block of the Device settings section (`ExitLoadField`), checked on the whole page on a fake node.
 * Contract: the block is `#exit-load` (only when `exit.load` is in the snapshot of the network), the count `#exit-load-streams`, the
 * warning `#exit-load-warning`, the table `#exit-load-table`, the field `#exit-load-limit`, its status `#exit-load-status`.
 * The limit is saved to the config of the main network, from the tab of any network. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createFakeNode } from "../testing/fake-node.mjs";
import { NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, settle, unmountPage } from "../testing/page.mjs";
import { baseConfig, PEER2_FULL, PEER_FULL } from "../testing/section.mjs";

const LOAD = {
  streams: 20, limit: 20, limit_auto: 150, overloaded: "limit", admitted: 40, refused: 9,
  members: [
    { id_full: PEER2_FULL, admitted: 0, refused: 0 },
    { id_full: PEER_FULL, admitted: 10, refused: 6 },
    { id_full: "D".repeat(64), admitted: 2, refused: 3 },
  ],
  elsewhere_refused: 4,
};

beforeEach(() => localStorage.clear());
afterEach(() => unmountPage());

describe("ExitLoadField", () => {
  it("ExitLoadField hides when exit is off", async () => {
    const off = await open({ load: null });
    expect(off.block().exists()).toBe(false);
    expect(off.wrapper.find("#you-exit-iface").exists()).toBe(true);
    const gone = await open({ load: undefined });
    expect(gone.block().exists()).toBe(false);
  });

  it("shows the table and other-networks row", async () => {
    const page = await open({ load: LOAD });
    expect(page.text("#exit-load-streams")).toBe("Connections open through this computer: 20 of 20");
    const rows = page.wrapper.findAll("#exit-load-table .exit-load-row").map((r) => r.text());
    expect(rows).toEqual([
      "peer-one — 6 of 16 connections",
      "DDDDDDDD — 3 of 5 connections",
      "Members of your other networks — 4",
    ]);
    expect(page.text("#exit-load-table")).toContain("Turned away in the last 10 minutes");
    const calm = await open({ load: { ...LOAD, overloaded: null, refused: 0, members: [], elsewhere_refused: 0 } });
    expect(calm.wrapper.find("#exit-load-table").exists()).toBe(false);
  });

  it("warning text follows the reason", async () => {
    const texts = {
      limit: "This computer is turning members away: all 20 connections are in use. Raise the connection limit below.",
      share: "This computer is turning members away: a member has used up its share of the 20 connections. Raise the connection limit below.",
      memory: "This computer is turning members away: it is low on memory. Raising the limit will not help — fewer members or a machine with more memory will.",
    };
    for (const [reason, text] of Object.entries(texts)) {
      const page = await open({ load: { ...LOAD, overloaded: reason } });
      expect(page.text("#exit-load-warning")).toBe(text);
      unmountPage();
    }
    const none = await open({ load: { ...LOAD, overloaded: null } });
    expect(none.wrapper.find("#exit-load-warning").exists()).toBe(false);
  });

  it("saves exit.max_streams to the primary config from an added network tab", async () => {
    const page = await open({ load: LOAD, hash: "peers/" + NET_B + "/device", twoNetworks: true });
    await page.type("500");
    expect(page.puts("/api/config")).toHaveLength(1);
    expect(page.puts("/api/config")[0].config.exit.max_streams).toBe(500);
    expect(page.puts("/api/net/" + NET_B + "/config")).toHaveLength(0);
    expect(page.text("#exit-load-status")).toBe("Saved");
    expect(page.text("#exit-load")).toContain("(now 150)");
  });

  it("field is read-only when set at startup", async () => {
    const fixed = await open({ load: LOAD, readOnly: ["exit.max_streams"] });
    expect(fixed.field().attributes("disabled")).toBeDefined();
    expect(fixed.text("#exit-load-cli-note")).not.toBe("");
    unmountPage();
    const free = await open({ load: LOAD });
    expect(free.field().attributes("disabled")).toBeUndefined();
  });
});

async function open({ load, hash = "peers/main/device", twoNetworks = false, readOnly = [] }) {
  const opts = twoNets();
  const exit = load === undefined ? { enabled: true } : { enabled: true, load };
  const state = { ...opts.state, exit };
  const config = baseConfig({ exit: { enabled: true, max_streams: 0 } });
  const net = { ...opts.slots[1], state: { ...opts.slots[1].state, exit }, config: structuredClone(config) };
  const slots = twoNetworks ? [opts.slots[0], net] : [opts.slots[0]];
  const node = createFakeNode({ ...opts, state, config, slots });
  if (readOnly.length > 0) {
    node.routes["GET /api/config"] = () => ({ rev: node.rev, config: structuredClone(node.config), file: null, read_only: readOnly, restart_fields: [], restart_pending: [], cli_overrides: readOnly });
  }
  const { wrapper, store } = await mountPage(node, { hash });
  const text = (sel) => (wrapper.find(sel).exists() ? wrapper.get(sel).text().replace(/\s+/g, " ").trim() : "");
  const field = () => wrapper.find("#exit-load-limit");
  return {
    node, wrapper, store, text, field,
    block: () => wrapper.find("#exit-load"),
    puts: (path) => node.calls.filter((c) => c.method === "PUT" && c.path === path).map((c) => c.body),
    async type(value) {
      await field().setValue(value);
      await field().trigger("blur");
      await vi.advanceTimersByTimeAsync(0);
      await settle();
    },
  };
}
