/* The card of a removed computer: an invite for another network that waits for a member leads to the new tab when the slot arrives. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { resetNetBanner } from "../home/add-net-banner.ts";
import { createFakeNode } from "../testing/fake-node.mjs";
import { membersReply, peerOneRow, selfRow } from "../testing/members-fixtures.mjs";
import { NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, polls, settle, unmountPage } from "../testing/page.mjs";
import { baseConfig, baseState } from "../testing/section.mjs";

const FRESH = "neta22";
const INVITE = "mesh-invite2-ABCDEFGH";

beforeEach(() => {
  localStorage.clear();
  resetNetBanner();
});
afterEach(unmountPage);

describe("the card of a banned computer", () => {
  it("an invite to another network from the card of a banned one leads to the new tab", async () => {
    const page = await openBanned();
    await page.click("#members-removed-join");
    await page.type("#rejoin-input", INVITE);
    await page.click("#rejoin-btn");
    await polls(page.node, 1);
    page.arrives();
    await polls(page.node, 1);
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    await vi.advanceTimersByTimeAsync(0);
    await settle();
    expect(location.hash).toBe("#peers/" + FRESH);
  });
});

function becomesNetwork(slot) {
  slot.joining = false;
  slot.label = "work";
  slot.id_hex = "net-cccc";
  slot.state = baseState({ network: { id_hex: "net-cccc", masked: "mesh2-••••-••••" }, peers: [] });
  slot.routes["GET /api/members"] = membersReply({ rows: [selfRow()] });
}

async function openBanned() {
  const opts = twoNets();
  opts.slots[1].members = membersReply({ rows: [selfRow(), peerOneRow()], me_excluded: true });
  opts.slots.push({ id: FRESH });
  const node = createFakeNode({ ...opts, config: baseConfig({ ports: { published: [{ port: 8080, for: "all" }], opened: [] } }) });
  const slot = node.slotList.find((s) => s.id === FRESH);
  node.routes["POST /api/network/join"] = () => { slot.joining = true; return { label: null, restart: false, waiting: true, slot: FRESH }; };
  const { wrapper } = await mountPage(node, { hash: "peers/" + NET_B });
  const el = () => wrapper.get("#sec-peers");
  return {
    node,
    arrives: () => becomesNetwork(slot),
    async click(sel) {
      await el().get(sel).trigger("click");
      await settle();
    },
    async type(sel, value) {
      await el().get(sel).setValue(value);
      await settle();
    },
  };
}
