/* The header line of the shell takes the admission refusal from the server flag of the leading slot, after the hold delay. */

import { afterEach, beforeEach, describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { ADMISSION_REFUSED_DELAY_MS } from "../peers/peers-texts.ts";
import { build, flush } from "./use-shell-support.mjs";

beforeEach(() => vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] }));
afterEach(() => vi.useRealTimers());

const NET = { label: "family", id_hex: "3f2a01", short: "3f2a", members: 2 };
const net = (admission_refused) => ({
  id: "main", label: "family", id_hex: "3f2a01", status: "online", exit_enabled: false, proxy: true, route_state: "direct", exits: [],
  members_voting: 0, joining: false, members_admitting: 0, me_excluded: false, me_pending: false, peers_online: 0, peers_total: 1, admission_refused,
});

async function mounted(flag) {
  const env = build();
  await flush();
  env.store.lead = "main";
  env.store.state = { boot_id: "b1", config_rev: 1, node: { status: "running" }, network: NET, peers: [], route: { state: "direct", exits: [] } };
  env.store.nets = [net(flag)];
  return env;
}

describe("the header line and the refusal flag", () => {
  it("flag on: red \"Sites won't open\" only after the delay", async () => {
    const env = await mounted(true);
    assert.ok(!/^Sites won't open/.test(env.shell.status.value.text));
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS);
    assert.match(env.shell.status.value.text, /^Sites won't open/);
    assert.equal(env.shell.status.value.level, "red");
    env.unmount();
  });

  it("flag off: never", async () => {
    const env = await mounted(false);
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS * 2);
    assert.ok(!/^Sites won't open/.test(env.shell.status.value.text));
    env.unmount();
  });

  it("the flag drops: the line is not red any more", async () => {
    const env = await mounted(true);
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS);
    env.store.nets = [net(false)];
    assert.ok(!/^Sites won't open/.test(env.shell.status.value.text));
    env.unmount();
  });
});
