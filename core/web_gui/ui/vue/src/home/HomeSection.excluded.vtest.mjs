/* The excluded-computer card on Home: in a Managed network it says an admin removed the computer, in a Peer network
 * it keeps the "banned by members" wording. */

import { afterEach, describe, expect, it } from "vitest";
import { visibleText } from "../testing/dom.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { ANNA_FULL, ID_HEX, NET_NAME, managedMembers, managedState, nodeConfig, serveManaged } from "../testing/managed-fixtures.mjs";
import { excludedInfo, memberRow } from "../testing/members-fixtures.mjs";
import { openMembersPage, stateWith } from "../testing/members-page.mjs";
import { mountPage, settle, unmountPage } from "../testing/page.mjs";
import { membersReply, selfRow } from "../testing/members-fixtures.mjs";

afterEach(unmountPage);

const BANNED = "Members of the network banned it";
const REMOVED = "An admin removed it";

describe("a banned computer on Home", () => {
  it("in a Managed network says_an_admin_removed_it", async () => {
    const { wrapper } = await mountPage(managedExcludedNode(), { hash: "home" });
    await settle();
    const text = visibleText(wrapper.get("#excluded-card").element).replace(/\s+/g, " ");
    expect(text).toContain(REMOVED);
    expect(text).not.toContain(BANNED);
  });

  it("in a Peer network keeps_the_members_text", async () => {
    const page = await openMembersPage({
      members: membersReply({ me_excluded: true, members: [selfRow({ status: "excluded", excluded: excludedInfo() })] }),
      state: stateWith({ me_excluded: true }),
      tab: "home",
    });
    expect(page.text("#excluded-card")).toContain(BANNED);
    expect(page.text("#excluded-card")).not.toContain(REMOVED);
  });
});

function managedExcludedNode() {
  const state = managedState({ meRole: "member", admins: [ANNA_FULL], network: {} });
  state.members = { count: 1, gossip_neighbors: 0, voting: 0, me_excluded: true };
  const members = managedMembers(state);
  members.me_excluded = true;
  members.members = members.members.map((m) => (m.self ? memberRow({ ...m, status: "excluded", excluded: excludedInfo() }) : m));
  const node = createFakeNode({ state, config: nodeConfig(), slots: [{ id: "main", label: NET_NAME, id_hex: ID_HEX, members }] });
  serveManaged(node, { brief: { kind: "Managed", me_admin: false, can_invite: false, me_excluded: true } });
  return node;
}
