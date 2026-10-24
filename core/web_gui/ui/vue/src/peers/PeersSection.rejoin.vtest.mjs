/* The tab of a network this computer is out of: the card's button opens the one invite form under the card, in the same tab.
 * Network A is `main` ("friends"), network B is `k7q2xa` ("my devices"); in B the other members removed this computer or banned it. */

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { createFakeNode } from "../testing/fake-node.mjs";
import { membersReply, peerOneRow, selfRow } from "../testing/members-fixtures.mjs";
import { NET_A, NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, settle, unmountPage } from "../testing/page.mjs";
import { baseConfig, baseState } from "../testing/section.mjs";

const INVITE = "mesh-invite2-ABCDEFGH";
const REFUSED = "The members of the network refused this computer.";

beforeEach(() => localStorage.clear());
afterEach(unmountPage);

const removed = () => membersReply({ rows: [selfRow(), peerOneRow()], me_removed: true });
const banned = () => membersReply({ rows: [selfRow(), peerOneRow()], me_excluded: true });

describe("the tab of a removed or banned computer", () => {
  it("the button of the card opens the form in this tab and focuses its field", async () => {
    const page = await openTab({ members: removed() });
    await page.click("#members-removed-join");
    expect(page.has("#rejoin-card")).toBe(true);
    expect(page.has("#members-removed-join"), "one main button on the screen").toBe(false);
    expect(page.focusedId()).toBe("rejoin-input");
    expect(location.hash).toBe("#peers/" + NET_B);
    expect(page.text("#rejoin-title")).toBe('Join "my devices" again');
  });

  it("a ban in a Peer network offers to join another network", async () => {
    const page = await openTab({ members: banned() });
    await page.click("#members-removed-join");
    expect(page.text("#rejoin-title")).toBe("Join another network");
  });

  it("the answer with the slot of the tab says so in the form", async () => {
    const page = await openTab({ members: removed() });
    page.node.routes["POST /api/network/join"] = { label: "my devices", restart: false, slot: NET_B };
    await page.click("#members-removed-join");
    await page.type("#rejoin-input", INVITE);
    await page.click("#rejoin-btn");
    expect(page.node.calls.filter((c) => c.method === "POST").map((c) => c.path)).toContain("/api/network/join");
    expect(page.text("#rejoin-status")).not.toBe("");
  });

  it("a join that the members refused afterwards is told under the card", async () => {
    const page = await openTab({ members: banned(), refused: true });
    expect(page.text("#rejoin-refused")).toBe(REFUSED);
    await page.click("#members-removed-join");
    expect(page.text("#rejoin-refused")).toBe(REFUSED);
  });

  it("another tab closes the form", async () => {
    const page = await openTab({ members: removed() });
    await page.click("#members-removed-join");
    location.hash = "peers/" + NET_A;
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    await settle();
    expect(page.has("#rejoin-card")).toBe(false);
  });

  it("the page never repeats an id", async () => {
    const page = await openTab({ members: removed() });
    await page.click("#members-removed-join");
    const ids = [...page.wrapper.element.querySelectorAll("[id]")].map((el) => el.id);
    expect(ids.filter((id, i) => ids.indexOf(id) !== i)).toEqual([]);
  });
});

async function openTab({ members, refused = false }) {
  const opts = twoNets();
  const b = opts.slots[1];
  b.members = members;
  if (refused) b.state = { ...b.state, node: { ...b.state.node, invite_join: { state: "refused", message: REFUSED } } };
  const node = createFakeNode({ ...opts, config: baseConfig({ ports: { published: [{ port: 8080, for: "all" }], opened: [] } }) });
  const { wrapper } = await mountPage(node, { hash: "peers/" + NET_B });
  const el = () => wrapper.get("#sec-peers");
  return {
    node, wrapper,
    has: (sel) => el().find(sel).exists(),
    text: (sel) => el().get(sel).text().replace(/\s+/g, " ").trim(),
    focusedId: () => document.activeElement?.id,
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
