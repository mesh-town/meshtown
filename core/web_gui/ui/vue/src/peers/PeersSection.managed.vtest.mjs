/* The Members tab of a Managed network, the whole page on a fake node: the Admin badge, the line of admins (`#admins-line`),
 * the plaque instead of the invite button (`#peers-invite-note`), the "Delete network for everyone…" entry (`#you-delete-net-btn`
 * in the "You" block, which shows with several networks) and the card of an excluded computer with the admin's name.
 * Network "work" is the single one unless a test says two; names: this computer "finch-unwary", admins "anna" and (when listed) "bob". */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { visibleText } from "../testing/dom.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { ANNA_FULL, BOB_FULL, ID_HEX, ME_FULL, NET_NAME, managedMembers, managedNode, managedState, nodeConfig, serveManaged } from "../testing/managed-fixtures.mjs";
import { excludedInfo, memberRow } from "../testing/members-fixtures.mjs";
import { NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, settle, unmountPage } from "../testing/page.mjs";

beforeEach(() => localStorage.clear());
afterEach(() => unmountPage());

describe("the Admin badge", () => {
  it("is on every admin row in a Managed network, this computer's own row included", async () => {
    const page = await open(managedNode({ meRole: "admin", admins: [ME_FULL, ANNA_FULL] }));
    expect(page.badgesOf("finch-unwary")).toContain("Admin");
    expect(page.badgesOf("anna")).toContain("Admin");
  });

  it("is not on a plain member", async () => {
    const page = await open(managedNode({ meRole: "admin", admins: [ME_FULL, ANNA_FULL] }));
    expect(page.badgesOf("bob")).not.toContain("Admin");
  });

  it("a member computer sees it on the admins' rows only", async () => {
    const page = await open(managedNode({ meRole: "member", admins: [ANNA_FULL] }));
    expect(page.badgesOf("anna")).toContain("Admin");
    expect(page.badgesOf("finch-unwary")).not.toContain("Admin");
  });

});

describe("the line of admins", () => {
  it("names this computer you and the others by name", async () => {
    const page = await open(managedNode({ meRole: "admin", admins: [ME_FULL, ANNA_FULL] }));
    expect(page.adminsLine()).toBe("Managed network · Admins: you, anna");
  });

  it("a member computer sees the names only", async () => {
    const page = await open(managedNode({ meRole: "member", admins: [ANNA_FULL, BOB_FULL] }));
    expect(page.adminsLine()).toBe("Managed network · Admins: anna, bob");
  });
});

describe("the invitation", () => {
  describe("a member of a Managed network", () => {
    it("sees the plaque with the admins, without the button", async () => {
      const page = await open(managedNode({ meRole: "member", admins: [ANNA_FULL, BOB_FULL] }));
      expect(page.inviteNote()).toBe('Only admins can invite people to "work". Ask anna or bob to send an invite.');
      expect(page.inviteButton().exists()).toBe(false);
    });

    it("one admin makes it Ask <a> to send an invite.", async () => {
      const page = await open(managedNode({ meRole: "member", admins: [ANNA_FULL] }));
      expect(page.inviteNote()).toBe('Only admins can invite people to "work". Ask anna to send an invite.');
    });

    it("an admin with no name goes by the short id", async () => {
      const page = await open(managedNode({ meRole: "member", admins: [ANNA_FULL, "d3d3d3d3" + "0".repeat(56)] }));
      expect(page.inviteNote()).toContain("d3d3d3d3");
    });
  });

  describe("an admin", () => {
    it("keeps the button and has no plaque", async () => {
      const page = await open(managedNode({ meRole: "admin" }));
      expect(page.inviteButton().exists()).toBe(true);
      expect(page.wrapper.find("#peers-invite-note").exists()).toBe(false);
    });
  });
});

describe("deleting the network", () => {
  describe("an admin with several networks", () => {
    it("sees the red entry in the You block", async () => {
      const page = await openTwoManaged({ meRole: "admin" });
      expect(page.deleteButton().text()).toBe("Delete network for everyone…");
      expect(page.deleteButton().classes()).toContain("btn-danger");
    });

    it("the entry opens a dialog that asks for the name", async () => {
      const page = await openTwoManaged({ meRole: "admin" });
      await page.whenDeleteClicked();
      expect(page.wrapper.find("#member-reason").exists(), "the typed-name field").toBe(true);
      expect(page.wrapper.get("#member-confirm-yes").attributes("disabled"), "not before the name is typed").toBeDefined();
    });

    it("confirming sends the delete with confirm:true to this network", async () => {
      const page = await openTwoManaged({ meRole: "admin" });
      await page.whenDeleteConfirmed();
      const sent = page.node.calls.filter((c) => c.method === "POST" && c.path.endsWith("/network/delete"));
      expect(sent).toHaveLength(1);
      expect(sent[0].body).toEqual({ confirm: true });
      expect(sent[0].path).toContain(NET_B);
    });
  });

  describe("after the deletion", () => {
    it("the admin reads that the network is gone and dismisses it with OK", async () => {
      const page = await openTwoManaged({ meRole: "admin" });
      await page.whenDeleteConfirmed();
      expect(page.wrapper.get("#deleted-net-banner").text()).toContain('You deleted "work". All members have left it.');
      await page.wrapper.get("#deleted-net-ok").trigger("click");
      expect(page.wrapper.find("#deleted-net-banner").exists()).toBe(false);
    });
  });

  describe("a member", () => {
    it("has no entry", async () => {
      const page = await openTwoManaged({ meRole: "member", admins: [ANNA_FULL] });
      expect(page.wrapper.find("#you-delete-net-btn").exists()).toBe(false);
    });
  });
});

describe("a computer excluded by an admin", () => {
  it("is told who removed it and how to come back", async () => {
    const page = await open(excludedNode({ excluded_by: { id: ANNA_FULL, name: "anna" } }));
    expect(page.pageText()).toContain('An admin (anna) removed this computer from "work". To come back, ask an admin for a new invite.');
  });

  it("a bare id string names the admin as well", async () => {
    const page = await open(excludedNode({ excluded_by: "b1b1b1b1" }));
    expect(page.pageText()).toContain("An admin (b1b1b1b1) removed this computer");
  });

  it("without excluded_by keeps the old text", async () => {
    const page = await open(excludedNode({}));
    expect(page.pageText()).not.toContain("An admin (");
  });
});

function excludedNode(network) {
  const state = managedState({ meRole: "member", admins: [ANNA_FULL], network });
  state.members = { count: 1, gossip_neighbors: 0, me_excluded: true };
  const members = managedMembers(state);
  members.me_excluded = true;
  members.members = members.members.map((m) => (m.self ? memberRow({ ...m, status: "excluded", excluded: excludedInfo() }) : m));
  const node = createFakeNode({ state, config: nodeConfig(), slots: [{ id: "main", label: NET_NAME, id_hex: ID_HEX, members }] });
  serveManaged(node, { brief: { me_admin: false, can_invite: false, me_excluded: true } });
  return node;
}

// A node with the plain first network and the Managed "work" as the second one.
function openTwoManaged({ meRole, admins }) {
  const state = managedState({ meRole, admins, network: { id_hex: "net-bbbb" } });
  const opts = twoNets({ label: NET_NAME, state, members: managedMembers(state), id_hex: "net-bbbb" });
  const node = createFakeNode({ ...opts, config: nodeConfig() });
  serveManaged(node, { briefs: { [NET_B]: { me_admin: meRole === "admin", can_invite: meRole === "admin" } } });
  node.routes["POST /api/network/delete"] = {};
  node.slotList.filter((s) => s !== node).forEach((s) => (s.routes["POST /api/network/delete"] = {}));
  return open(node, "peers/" + NET_B + "/device");
}

async function open(node, hash = "peers") {
  const { wrapper, store } = await mountPage(node, { hash });
  const section = () => wrapper.get("#sec-peers");
  const rowOf = (name) => wrapper.findAll("#peers-list > *").find((r) => r.text().includes(name));
  const click = async (w) => { await w.trigger("click"); await settle(); };
  return {
    node, wrapper, store,
    pageText: () => visibleText(section().element).replace(/\s+/g, " "),
    listText: () => wrapper.get("#peers-list").text(),
    badgesOf: (name) => (rowOf(name) ? rowOf(name).findAll(".peer-badge").map((b) => b.text()) : ["<no such row: " + name + ">"]),
    adminsLine: () => wrapper.get("#admins-line").text().replace(/\s+/g, " ").trim(),
    inviteNote: () => wrapper.get("#peers-invite-note").text().replace(/\s+/g, " ").trim(),
    inviteButton: () => wrapper.find("#peers-invite-btn"),
    deleteButton: () => wrapper.get("#you-delete-net-btn"),
    whenDeleteClicked: () => click(wrapper.get("#you-delete-net-btn")),
    async whenDeleteConfirmed() {
      await click(wrapper.get("#you-delete-net-btn"));
      await wrapper.get("#member-reason").setValue(NET_NAME);
      await settle();
      await click(wrapper.get("#member-confirm-yes"));
    },
  };
}

vi.setConfig({ testTimeout: 15000 });
