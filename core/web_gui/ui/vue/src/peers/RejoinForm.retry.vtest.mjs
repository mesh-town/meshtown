/* The form of a left network's tab: no waiting line before a refusal the page foresees, and a new attempt clears the old refusal. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { INSPECT_DELAY_MS } from "../home/inspect-preview.ts";
import { resetNetBanner } from "../home/add-net-banner.ts";
import { createFakeNode } from "../testing/fake-node.mjs";
import { membersReply, peerOneRow, selfRow } from "../testing/members-fixtures.mjs";
import { NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, settle, unmountPage } from "../testing/page.mjs";
import { baseConfig } from "../testing/section.mjs";

const INVITE = "mesh-invite2-ABCDEFGH";
const REFUSED = { state: "refused", message: "The members of the network refused this computer.", label: "my devices", expires_unix: 1 };
const WAITING = { state: "waiting", message: "waiting", label: "my devices", expires_unix: 1 };

beforeEach(() => {
  localStorage.clear();
  resetNetBanner();
});
afterEach(unmountPage);

const banned = () => membersReply({ rows: [selfRow(), peerOneRow()], me_excluded: true });

describe("the join from the form of a removed computer", () => {
  it("a network that removed the computer: the waiting line stays", async () => {
    const page = await openTab({ members: banned(), inspect: { label: "my devices", id_hex: "net-bbbb", place: "return", slot: NET_B } });
    await page.send();
    expect(page.text("#rejoin-status")).toContain("Invite sent");
  });

  it("an invite to another network: the waiting line stays", async () => {
    const page = await openTab({ members: banned(), inspect: { label: "work", id_hex: "net-cccc", place: "new" } });
    await page.send();
    expect(page.text("#rejoin-status")).toContain("Invite sent");
  });

  it("a new attempt clears the refusal on the tab and shows the waiting line", async () => {
    const page = await openTab({ members: banned(), invite_join: REFUSED });
    expect(page.has("#rejoin-refused")).toBe(true);
    await page.openForm();
    await page.send();
    expect(page.has("#rejoin-refused")).toBe(false);
    expect(page.text("#rejoin-status")).toContain("Invite sent");
  });

  it("the same refusal after waiting shows again", async () => {
    const page = await openTab({ members: banned(), invite_join: REFUSED });
    await page.openForm();
    await page.send();
    await page.snapshot(WAITING);
    expect(page.has("#rejoin-refused")).toBe(false);
    await page.snapshot(REFUSED);
    expect(page.has("#rejoin-refused")).toBe(true);
  });

  it("an empty field: the click does not clear the refusal", async () => {
    const page = await openTab({ members: banned(), invite_join: REFUSED });
    await page.openForm();
    await page.click("#rejoin-btn");
    expect(page.has("#rejoin-refused")).toBe(true);
  });
});

async function openTab({ members, inspect = null, invite_join = null }) {
  const opts = twoNets();
  const b = opts.slots[1];
  b.members = members;
  const excluded = members.me_excluded === true;
  b.state = { ...b.state, network: { ...b.state.network }, members: { count: 2, gossip_neighbors: 0, me_excluded: excluded }, node: { ...b.state.node, invite_join } };
  const node = createFakeNode({ ...opts, config: baseConfig({ ports: { published: [{ port: 8080, for: "all" }], opened: [] } }) });
  node.routes["POST /api/network/join"] = { label: "my devices", restart: false, waiting: true, slot: NET_B };
  if (inspect) node.routes["POST /api/network/inspect"] = inspect;
  const { wrapper } = await mountPage(node, { hash: "peers/" + NET_B });
  const el = () => wrapper.get("#sec-peers");
  const page = {
    node, wrapper,
    has: (sel) => el().find(sel).exists(),
    text: (sel) => el().get(sel).text().replace(/\s+/g, " ").trim(),
    hidden: (sel) => !el().find(sel).exists() || el().get(sel).attributes("hidden") !== undefined,
    async click(sel) {
      await el().get(sel).trigger("click");
      await settle();
    },
    async openForm() {
      if (!page.has("#rejoin-card")) await page.click("#members-removed-join");
    },
    async send() {
      await page.openForm();
      await el().get("#rejoin-input").setValue(INVITE);
      await vi.advanceTimersByTimeAsync(INSPECT_DELAY_MS);
      await settle();
      await page.click("#rejoin-btn");
    },
    async snapshot(invite_join) {
      const slot = node.slotList.find((s) => s.id === NET_B);
      slot.state = { ...slot.state, node: { ...slot.state.node, invite_join } };
      await vi.advanceTimersByTimeAsync(5000);
      await settle();
    },
  };
  return page;
}
