/* Removing offline members and being removed, on a node with two networks: the whole page on a fake node with two slots.
 * Network A is `main` ("friends"), network B is `k7q2xa` ("my devices"). The groups under the list (`#stale-list`,
 * `#forgotten-list`) and the card `#excluded-card` are built from the member list of the SELECTED tab; an action goes to the
 * address of that tab's network (`/api/net/<slot>/members/forget`). The components: `members/StaleList.vtest.mjs`,
 * `members/ForgottenList.vtest.mjs`, `members/ExcludedCard.vtest.mjs`. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createFakeNode } from "../testing/fake-node.mjs";
import { actionReply, membersReply, peerOneRow, selfRow, staleRow } from "../testing/members-fixtures.mjs";
import { NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, settle, unmountPage } from "../testing/page.mjs";

const DAY = 86400;
const OLD = staleRow("old-desktop", 9 * DAY);

beforeEach(() => localStorage.clear());
afterEach(unmountPage);

describe("two networks, the second has an old member", () => {
  describe("groups follow selected tab", () => {
    it("the first tab has no stale group", async () => {
      const page = await openTwo();
      expect(page.has("#stale-list")).toBe(false);
    });

    it("the second tab shows it with the old member", async () => {
      const page = await openTwo();
      await page.whenTabClicked("my devices");
      expect(page.text("#stale-list > summary")).toBe("Offline for over a week (1)");
      expect(page.text("#stale-list")).toContain("old-desktop");
      expect(page.text("#stale-list")).toContain("Last online 9 d ago");
    });

    it("going back to the first tab hides the group again", async () => {
      const page = await openTwo();
      await page.whenTabClicked("my devices");
      await page.whenAddressBack();
      expect(page.has("#stale-list")).toBe(false);
    });
  });

  describe("forget from stale posts to tab slot", () => {
    it("the removal goes to the network of the selected tab", async () => {
      const page = await openTwo();
      await page.whenTabClicked("my devices");
      await page.whenStaleMemberRemoved(OLD.id_full);
      expect(page.posted("/api/net/" + NET_B + "/members/forget")).toEqual([{ id: OLD.id_full }]);
      expect(page.posted("/api/members/forget"), "nothing for the first network").toEqual([]);
    });

    it("the dialog asks to propose the removal", async () => {
      const page = await openTwo();
      await page.whenTabClicked("my devices");
      await page.whenStaleMenuItemPicked(OLD.id_full);
      expect(page.text("#confirm-title")).toBe("Remove old-desktop from the network?");
      expect(page.text("#member-confirm-yes")).toBe("Propose removal");
    });

    it("the member leaves the stale group and appears among the removed one", async () => {
      const page = await openTwo({ afterForget: removedRows() });
      await page.whenTabClicked("my devices");
      await page.whenStaleMemberRemoved(OLD.id_full);
      expect(page.has("#stale-list")).toBe(false);
      expect(page.text("#forgotten-list > summary")).toBe("Removed from the member list (1)");
    });
  });
});

describe("card when slot me removed", () => {
  it("the first tab keeps the list while only the second network removed this computer", async () => {
    const page = await openTwo({ removedInB: true });
    expect(page.has("#excluded-card")).toBe(false);
    expect(page.has("#peers-list")).toBe(true);
  });

  it("the second tab shows the card instead of the list", async () => {
    const page = await openTwo({ removedInB: true });
    await page.whenTabClicked("my devices");
    expect(page.text("#excluded-card")).toContain('This computer was removed from the member list "my devices"');
    expect(page.has("[data-member-menu]")).toBe(false);
  });

  it("the card offers joining with an invite", async () => {
    const page = await openTwo({ removedInB: true });
    await page.whenTabClicked("my devices");
    expect(page.text("#excluded-join")).toBe("Join with an invite");
  });

  it("going back to the first tab shows its list again", async () => {
    const page = await openTwo({ removedInB: true });
    await page.whenTabClicked("my devices");
    await page.whenAddressBack();
    expect(page.has("#excluded-card")).toBe(false);
    expect(page.has("#peers-list")).toBe(true);
  });
});

// The reply of B: you, peer-one and an old member; `removedInB` - the members removed this computer.
const repliesOfB = (rows, over = {}) => membersReply({ rows, ...over });
const baseRowsOfB = () => [selfRow(), peerOneRow(), OLD];
// What the server answers after the removal was proposed and confirmed: the old member is in the removed group.
const removedRows = () => repliesOfB([selfRow(), peerOneRow(), { ...OLD, status: "forgotten", forget: forgetDone() }]);
const forgetDone = () => ({ votes: 2, need: 2, proposed_by: "aaaa1111", voters: ["aaaa1111", "bbbb2222"], mine: true, at_unix: 1792400000 });

async function openTwo({ removedInB = false, afterForget = null } = {}) {
  const b = repliesOfB(baseRowsOfB(), removedInB ? { me_removed: true } : {});
  const opts = twoNets({ members: b });
  const node = createFakeNode(opts);
  const reply = afterForget ?? repliesOfB(baseRowsOfB());
  node.slotList[1].routes["POST /api/members/forget"] = () => {
    node.slotList[1].routes["GET /api/members"] = reply;
    return actionReply(reply);
  };
  const { wrapper, store } = await mountPage(node, { hash: "peers" });
  return pageOf(node, wrapper, store);
}

function pageOf(node, wrapper, store) {
  const compact = (text) => text.replace(/\s+/g, " ").trim();
  const click = async (w) => { await w.trigger("click"); await settle(); };
  const tabNamed = (name) => wrapper.findAll("#sec-peers .ntabs a").find((t) => compact(t.text()).startsWith(name));
  const hashChanged = async () => {
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    await vi.advanceTimersByTimeAsync(0);
    await settle();
  };
  const text = (selector) => compact(wrapper.get(selector).text());
  return {
    node, wrapper, store, text,
    has: (selector) => wrapper.find(selector).exists(),
    posted: (path) => node.calls.filter((c) => c.method === "POST" && c.path === path).map((c) => c.body),
    async whenTabClicked(name) {
      const tab = tabNamed(name);
      await tab.trigger("click");
      if (location.hash !== tab.attributes("href")) location.hash = tab.attributes("href");
      await hashChanged();
    },
    async whenAddressBack() { location.hash = "peers"; await hashChanged(); },
    async whenStaleMenuItemPicked(idFull) {
      await click(wrapper.get('#stale-list button[data-member-menu="' + idFull + '"]'));
      await click(wrapper.get('[role="menu"] [data-member-item="forget"]'));
    },
    async whenStaleMemberRemoved(idFull) {
      await this.whenStaleMenuItemPicked(idFull);
      await click(wrapper.get("#member-confirm-yes"));
    },
  };
}
