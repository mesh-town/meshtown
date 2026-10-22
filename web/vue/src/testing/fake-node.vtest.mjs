/* Self-test of the fake node: the helper the moved-over cases rely on is itself checked. */

import { describe, expect, it } from "vitest";
import { demoConfig, demoState } from "./demo-node.mjs";
import { createFakeNode } from "./fake-node.mjs";

const make = () => createFakeNode({ state: demoState(), config: demoConfig() });
const json = (res) => res.json();

describe("fake-node", () => {
  it("the snapshot is a new object each time, with config_rev", async () => {
    const node = make();
    const a = await json(await node.fetch("/api/state"));
    const b = await json(await node.fetch("/api/state"));
    expect(a).toEqual(b);
    expect(a).not.toBe(b);
    expect(a.config_rev).toBe(1);
  });

  it("PUT on a stale rev gives 409 stale_rev with the current document; on the right one, rev + 1", async () => {
    const node = make();
    node.rev = 3;
    const stale = await node.fetch("/api/config", { method: "PUT", body: JSON.stringify({ rev: 2, config: {} }) });
    expect(stale.status).toBe(409);
    expect(await json(stale)).toMatchObject({ error: "stale_rev", rev: 3, config: node.config });
    const ok = await node.fetch("/api/config", { method: "PUT", body: JSON.stringify({ rev: 3, config: { route: { exits: [] } } }) });
    expect(ok.status).toBe(200);
    expect(node.rev).toBe(4);
  });

  it('fail = "down" — fetch throws TypeError', async () => {
    const node = make();
    node.fail = "down";
    await expect(node.fetch("/api/state")).rejects.toBeInstanceOf(TypeError);
  });

  it("an unknown path gives 404 and an entry in unexpected", async () => {
    const node = make();
    const res = await node.fetch("/api/nope");
    expect(res.status).toBe(404);
    expect(node.unexpected).toEqual(["GET /api/nope"]);
  });
});
