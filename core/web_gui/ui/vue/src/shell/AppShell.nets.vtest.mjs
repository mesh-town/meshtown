/* The requests-to-confirm counter of the shell with several networks: the "Networks" tab chip and the header link `#strip-vote`
 * count the requests of every network together. Each join request counts as one, for an admin only;
 * a network the computer was excluded from adds nothing. The link goes to the one network that has requests, otherwise to `#peers`.
 * With a single network the counter is the one the page always had. */

import { afterEach, describe, expect, it } from "vitest";
import { membersReply, peerOneRow, pendingRow, selfRow } from "../testing/members-fixtures.mjs";
import { openMembersPage, stateWith } from "../testing/members-page.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, unmountPage } from "../testing/page.mjs";

afterEach(unmountPage);

const CHIP = "#tab-peers .chip";
const LINK = "#strip-vote";

describe("two networks", () => {
  describe("a request waits in each", () => {
    it("the tab chip shows the sum: 1 + 2 = 3", async () => {
      const page = await openTwoNets({ a: oneRequest(), b: twoRequests() });
      expect(page.chip()).toBe("3");
      expect(page.chipLabel()).toBe("3 requests need your confirmation");
    });

    it("the header link says 3 requests and leads to the common list", async () => {
      const page = await openTwoNets({ a: oneRequest(), b: twoRequests() });
      expect(page.linkText()).toBe("3 requests to confirm");
      expect(page.linkHref(), "requests in both networks: no single tab to go to").toBe("#peers");
    });
  });

  describe("requests only in the second network", () => {
    it("the chip counts them", async () => {
      const page = await openTwoNets({ a: noRequests(), b: twoRequests() });
      expect(page.chip()).toBe("2");
    });

    it("the header link leads to the tab of that network", async () => {
      const page = await openTwoNets({ a: noRequests(), b: twoRequests() });
      expect(page.linkText()).toBe("2 requests to confirm");
      expect(page.linkHref()).toBe("#peers/" + NET_B);
    });
  });

  describe("no requests anywhere", () => {
    it("neither the chip nor the link is shown", async () => {
      const page = await openTwoNets({ a: noRequests(), b: noRequests() });
      expect(page.hasChip()).toBe(false);
      expect(page.hasLink()).toBe(false);
    });
  });

  describe("the computer is excluded from the second network", () => {
    it("its requests add nothing to the sum", async () => {
      const page = await openTwoNets({ a: oneRequest(), b: twoRequests({ me_excluded: true }) });
      expect(page.chip()).toBe("1");
      expect(page.linkText()).toBe("1 request to confirm");
    });

    it("when it is the only network with requests, no chip and no link", async () => {
      const page = await openTwoNets({ a: noRequests(), b: twoRequests({ me_excluded: true }) });
      expect(page.hasChip()).toBe(false);
      expect(page.hasLink()).toBe(false);
    });
  });
});

describe("one network", () => {
  it("the chip and the link count its requests as before", async () => {
    const page = await openMembersPage({ members: twoRequests(), state: stateWith({ admitting: 2 }), tab: null });
    expect(page.text(CHIP)).toBe("2");
    expect(page.text(LINK)).toBe("2 requests to confirm");
    expect(page.attr(LINK, "href")).toBe("#peers");
  });

  it("an exclusion of this computer zeroes the counter", async () => {
    const rows = [selfRow(), peerOneRow(), pendingRow()];
    const page = await openMembersPage({ members: membersReply({ rows, me_excluded: true, me_admin: true }), state: stateWith({ admitting: 1 }), tab: null });
    expect(page.has(CHIP)).toBe(false);
    expect(page.has(LINK)).toBe(false);
  });
});

// A member list without requests.
const noRequests = (over = {}) => membersReply({ me_admin: true, ...over });

// One join request for this admin.
const oneRequest = (over = {}) => membersReply({ rows: [selfRow(), peerOneRow(), pendingRow()], me_admin: true, ...over });

// Two join requests for this admin.
const twoRequests = (over = {}) =>
  membersReply({ rows: [selfRow(), peerOneRow(), pendingRow(), pendingRow({ id: "ffff6666", id_full: "F".repeat(64) })], me_admin: true, ...over });

// The page on a node with two networks: `a` and `b` are the member lists of the first and the second one.
async function openTwoNets({ a, b }) {
  localStorage.clear();
  const opts = twoNets({ members: b });
  opts.slots[0].members = a;
  const node = createFakeNode(opts);
  const { wrapper } = await mountPage(node);
  const link = () => wrapper.find(LINK);
  return {
    node,
    wrapper,
    hasChip: () => wrapper.find(CHIP).exists(),
    hasLink: () => link().exists(),
    chip: () => wrapper.get(CHIP).text(),
    chipLabel: () => wrapper.get(CHIP).attributes("aria-label"),
    linkText: () => link().text(),
    linkHref: () => link().attributes("href"),
  };
}

