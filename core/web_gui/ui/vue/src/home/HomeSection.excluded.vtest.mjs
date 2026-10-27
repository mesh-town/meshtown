/* The excluded-computer card on Home: it says an admin removed the computer. */

import { afterEach, describe, expect, it } from "vitest";
import { visibleText } from "../testing/dom.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { ANNA_FULL, ID_HEX, NET_NAME, managedMembers, managedState, nodeConfig, serveManaged } from "../testing/managed-fixtures.mjs";
import { excludedInfo, memberRow } from "../testing/members-fixtures.mjs";
import { openMembersPage, stateWith } from "../testing/members-page.mjs";
import { mountPage, settle, unmountPage } from "../testing/page.mjs";
import { membersReply, selfRow } from "../testing/members-fixtures.mjs";

afterEach(unmountPage);

const REMOVED = "An admin removed it";
const squash = (el) => visibleText(el).replace(/\s+/g, " ").trim();

describe("a banned computer on Home", () => {
  it("Home: no card button, the only primary button is the Join of the form", async () => {
    const page = await openMembersPage({
      members: membersReply({ me_excluded: true, members: [selfRow({ status: "excluded", excluded: excludedInfo() })] }),
      state: stateWith({ me_excluded: true }),
      tab: "home",
    });
    expect(page.wrapper.find("#excluded-join").exists()).toBe(false);
    expect(page.wrapper.get("#join-btn").classes()).toContain("btn-primary");
    expect(page.wrapper.get("#home-root").findAll(".btn-primary")).toHaveLength(1);
  });

  it("says_an_admin_removed_it", async () => {
    const { wrapper } = await mountPage(managedExcludedNode(), { hash: "home" });
    await settle();
    const text = visibleText(wrapper.get("#excluded-card").element).replace(/\s+/g, " ");
    expect(text).toContain(REMOVED);
      expect(text).toContain("ask a network admin for a new invite");
    expect(text).not.toContain("contact a member");
  });

  it("says_removed_in_the_title_and_the_status_line", async () => {
    const { wrapper } = await mountPage(managedExcludedNode(), { hash: "home" });
    await settle();
    const title = wrapper.get("#excluded-card h2").text();
    expect(title).toBe(`This computer was removed from the network "${NET_NAME}"`);
    expect(squash(wrapper.get("#status-line").element)).toBe(`Removed from the network "${NET_NAME}" by an admin \u2014 sites won't open through Mesh Town`);
    expect(title).not.toContain("banned");
  });

});

function managedExcludedNode() {
  const state = managedState({ meRole: "member", admins: [ANNA_FULL], network: { label: NET_NAME } });
  state.members = { count: 1, gossip_neighbors: 0, me_excluded: true };
  const members = managedMembers(state);
  members.me_excluded = true;
  members.members = members.members.map((m) => (m.self ? memberRow({ ...m, status: "excluded", excluded: excludedInfo() }) : m));
  const node = createFakeNode({ state, config: nodeConfig(), slots: [{ id: "main", label: NET_NAME, id_hex: ID_HEX, members }] });
  serveManaged(node, { brief: { me_admin: false, can_invite: false, me_excluded: true } });
  return node;
}
