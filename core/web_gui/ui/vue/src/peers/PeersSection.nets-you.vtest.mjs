/* The "Networks" tab on a node with several networks: the "Rename" entry points and leaving a network.
 * Network A is `main` ("friends", peer-one and peer-two), network B is `k7q2xa` ("my devices", one other member: old-laptop).
 * Contract: with two or more networks "Rename" (the link of your row and the item of its menu) opens Device settings of the
 * network on show (`#peers/<slot>/device`) and focuses the name field of the "You in" block, `#you-name`; with one network it opens Home (`#home`) as before.
 * Leaving B sends one `POST /api/net/k7q2xa/members/leave {confirm:true}`, drops the tab of B and keeps the phrase under the list. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createFakeNode } from "../testing/fake-node.mjs";
import { EXTRA_FULL, extraRow, membersReply, selfRow } from "../testing/members-fixtures.mjs";
import { NET_A, NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, polls, settle, unmountPage } from "../testing/page.mjs";
import { SELF_FULL, baseConfig, baseState } from "../testing/section.mjs";

const LEFT_B = 'You left the network "my devices".';
const NOT_TOLD = "Mesh Town couldn't let the other members know, so for a while they may still see this computer in their list as offline. You don't need to do anything.";

beforeEach(() => localStorage.clear());
afterEach(unmountPage);

describe("rename", () => {
  describe("two networks", () => {
    it("the link of your row opens Device settings of the network on show and focuses the name field", async () => {
      const page = await setup({ hash: "peers/" + NET_B });
      await page.whenRenameLinkClicked();
      expect(location.hash).toBe("#peers/" + NET_B + "/device");
      expect(page.has("#you-block"), "Device settings are open").toBe(true);
      expect(page.focusedId()).toBe("you-name");
    });

    it("the menu item opens Device settings and focuses the name field", async () => {
      const page = await setup({ hash: "peers/" + NET_B });
      await page.whenMenuItemPicked("rename");
      expect(location.hash).toBe("#peers/" + NET_B + "/device");
      expect(page.focusedId()).toBe("you-name");
    });

    it("works on the first network too", async () => {
      const page = await setup();
      await page.whenRenameLinkClicked();
      expect(location.hash).toBe("#peers/main/device");
      expect(page.focusedId()).toBe("you-name");
    });

    it("already in Device settings — just focuses the name field", async () => {
      const page = await setup({ hash: "peers/" + NET_B + "/device" });
      expect(page.has("#you-block")).toBe(true);
      expect(page.has("#peers-list"), "there is no member row to rename from").toBe(false);
    });
  });

  describe("one network", () => {
    it("the link of your row still leads to Home", async () => {
      const page = await setup({ nets: 1 });
      expect(page.renameLink().attributes("href")).toBe("#home");
    });

    it("the menu item opens Home", async () => {
      const page = await setup({ nets: 1 });
      await page.whenMenuItemPicked("rename");
      expect(location.hash).toBe("#home");
      expect(page.has("#you-name"), "there is no \"You in\" block with one network").toBe(false);
    });
  });
});

describe("leaving the second network", () => {
  it("sends one confirmation to the address of that network and deletes nothing", async () => {
    const page = await setup({ hash: "peers/" + NET_B });
    await page.whenLeaveConfirmed();
    expect(page.leaveBodies(NET_B)).toEqual([{ confirm: true }]);
    expect(page.leaveBodies(NET_A), "the first network is not asked").toEqual([]);
    expect(page.callsOf("DELETE"), "no DELETE request").toEqual([]);
  });

  it("drops the tab of that network and shows the first one", async () => {
    const page = await setup({ hash: "peers/" + NET_B });
    await page.whenLeaveConfirmed();
    expect(page.tabNames().join(" ")).not.toContain("my devices");
    expect(page.codes()).toContain("bbbb2222");
    expect(page.codes(), "the members of the left network are gone").not.toContain("dddd4444");
  });

  it("says that you left, and the phrase stays after the next poll", async () => {
    const page = await setup({ hash: "peers/" + NET_B, announced: true });
    await page.whenLeaveConfirmed();
    expect(page.status()).toBe(LEFT_B);
    await polls(page.node, 1);
    expect(page.status()).toBe(LEFT_B);
  });

  it("adds the second sentence when the other members could not be told", async () => {
    const page = await setup({ hash: "peers/" + NET_B, announced: false });
    await page.whenLeaveConfirmed();
    expect(page.status()).toBe(LEFT_B + " " + NOT_TOLD);
  });

  describe("refused with 409 busy", () => {
    it("shows the sentence of the node in red under the list", async () => {
      const page = await setup({ hash: "peers/" + NET_B, refusal: "The node is busy, try again in a moment." });
      await page.whenLeaveConfirmed();
      expect(page.status()).toBe("The node is busy, try again in a moment.");
      expect(page.statusIsError()).toBe(true);
    });

    it("keeps the tab and the members of that network", async () => {
      const page = await setup({ hash: "peers/" + NET_B, refusal: "The node is busy, try again in a moment." });
      await page.whenLeaveConfirmed();
      expect(page.tabNames().join(" ")).toContain("my devices");
      expect(page.codes()).toContain("dddd4444");
      expect(page.callsOf("DELETE")).toEqual([]);
    });
  });
});

describe("leaving from Device settings", () => {
  it("\"Leave network…\" of the second network sends one confirmation to its address", async () => {
    const page = await setup({ hash: "peers/" + NET_B + "/device" });
    await page.whenLeaveFromDevice();
    expect(page.leaveBodies(NET_B)).toEqual([{ confirm: true }]);
    expect(page.leaveBodies(NET_A), "the first network is not asked").toEqual([]);
  });
});

describe("leaving the only network", () => {
  it("sends the confirmation to the plain address", async () => {
    const page = await setup({ nets: 1 });
    await page.whenLeaveConfirmed();
    expect(page.node.calls.filter((c) => c.method === "POST" && c.path === "/api/members/leave").map((c) => c.body)).toEqual([{ confirm: true }]);
  });

  it("the tab shows that there is no network, and the phrase is the usual one", async () => {
    const page = await setup({ nets: 1 });
    await page.whenLeaveConfirmed();
    await polls(page.node, 1);
    expect(page.has("#sec-peers [data-no-network-msg]"), "the \"no network\" card").toBe(true);
    expect(page.has("#peers-list")).toBe(false);
    expect(page.status()).toBe('You left the network "friends".');
  });
});

// A node with two networks (`nets: 2`) or with A alone (`nets: 1`), the page open on `hash`.
// `announced`: what the node says about telling the others (no value — what the fake node says); `refusal`: the leave of B is refused.
async function setup({ nets = 2, hash = "peers", announced = null, refusal = null } = {}) {
  const node = createFakeNode(nets === 2 ? withB() : alone());
  if (nets === 1) node.routes["POST /api/members/leave"] = () => leaveOfOnly(node);
  if (refusal) node.routes["POST /api/net/" + NET_B + "/members/leave"] = { status: 409, body: { error: "busy", message: refusal } };
  if (announced !== null) saying(node, announced);
  const { wrapper } = await mountPage(node, { hash });
  return pageOf(node, wrapper);
}

const selfPeer = () => ({ self: true, online: true, id: "aaaa1111", id_full: SELF_FULL, name: "finch-unwary", country: "RU", version: "0.1.0" });
const laptopPeer = () => ({ id: "dddd4444", id_full: EXTRA_FULL, name: "old-laptop", country: "NL", version: "0.1.0", online: true, path: "direct", rtt_ms: 20, exit: false });

function netB() {
  const base = baseState({ network: { id_hex: "net-bbbb", label: "my devices", masked: "mesh2-••••-••••" }, peers: [selfPeer(), laptopPeer()] });
  const state = { ...base, members: { count: 1, gossip_neighbors: 0, me_excluded: false } };
  return { state, members: membersReply({ rows: [selfRow(), extraRow()] }) };
}

// The published port of the fixture has no audience; real entries always have one.
const audienced = (opts) => ({ ...opts, config: baseConfig({ ports: { published: [{ port: 8080, for: "all" }], opened: [] } }) });

function withB() {
  const opts = audienced(twoNets(netB()));
  return { ...opts, state: { ...opts.state, network: { ...opts.state.network, label: "friends" } } };
}

function alone() {
  const opts = withB();
  return { ...opts, slots: [opts.slots[0]] };
}

// The node's answer to `members/leave` of B with a chosen `announced` (the fake node always says `false`).
function saying(node, announced) {
  const real = node.fetch;
  node.fetch = async (input, init) => {
    const res = await real(input, init);
    if (!String(input).endsWith("/members/leave") || !res.ok) return res;
    const body = { ...(await res.json()), announced };
    return new Response(JSON.stringify(body), { status: 200, headers: { "Content-Type": "application/json" } });
  };
}

// The only network is left: the node holds no network now and its brief has no key and no label.
function leaveOfOnly(node) {
  node.state = { ...node.state, network: null, peers: [], members: null };
  node.id_hex = null;
  node.label = null;
  node.routes["GET /api/members"] = { status: 409, body: { error: "busy", message: "The node is not in a network" } };
  return { ...membersReply(), left: true, announced: true, nets: [goneBrief()] };
}

const goneBrief = () => ({
  id: NET_A, label: null, id_hex: null, status: "running", exit_enabled: false, proxy: true, route_state: "direct", exits: [],
  joining: false, members_admitting: 0, me_excluded: false, me_pending: false, peers_online: 0, peers_total: 0,
});

function pageOf(node, wrapper) {
  const click = async (w) => { await w.trigger("click"); await settle(); };
  const hashChanged = async () => {
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    await vi.advanceTimersByTimeAsync(0);
    await settle();
  };
  const renameLink = () => wrapper.findAll("#peers-list .peer-self a").find((a) => a.text() === "Rename");
  const menuButton = () => wrapper.get('#peers-list button[data-member-menu="' + SELF_FULL + '"]');
  const whenMenuItemPicked = async (key) => {
    await click(menuButton());
    await click(wrapper.get('[role="menu"] [data-member-item="' + key + '"]'));
    await hashChanged();
  };
  return {
    node, wrapper, renameLink, whenMenuItemPicked,
    has: (selector) => wrapper.find(selector).exists(),
    focusedId: () => document.activeElement && document.activeElement.id,
    codes: () => wrapper.findAll("#sec-peers #peers-list .peer-code").map((c) => c.text()),
    tabNames: () => wrapper.findAll("#sec-peers .ntabs a").map((t) => t.text().replace(/\s+/g, " ").trim()),
    status: () => wrapper.get("#peers-exit-status").text(),
    statusIsError: () => wrapper.get("#peers-exit-status").classes().includes("field-error-text"),
    callsOf: (method) => node.calls.filter((c) => c.method === method),
    leaveBodies: (slot) => node.calls.filter((c) => c.method === "POST" && c.path === "/api/net/" + slot + "/members/leave").map((c) => c.body),
    whenRenameLinkClicked: async () => { await click(renameLink()); await hashChanged(); },
    whenLeaveFromDevice: async () => {
      await click(wrapper.get("#you-leave"));
      await click(wrapper.get("#member-confirm-yes"));
    },
    async whenLeaveConfirmed() {
      await whenMenuItemPicked("leave");
      await click(wrapper.get("#member-confirm-yes"));
    },
  };
}
