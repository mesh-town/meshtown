/* The pointer text in Settings with several networks: leaving and deleting are in Device settings of the network tab. */

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { createFakeNode } from "../testing/fake-node.mjs";
import { managedMembers, managedState, nodeConfig, serveManaged } from "../testing/managed-fixtures.mjs";
import { NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, unmountPage } from "../testing/page.mjs";

beforeEach(() => localStorage.clear());
afterEach(() => unmountPage());

async function pointer() {
  const state = managedState({ network: { id_hex: "net-bbbb" } });
  const node = createFakeNode({ ...twoNets({ label: "work", state, members: managedMembers(state), id_hex: "net-bbbb" }), config: nodeConfig() });
  serveManaged(node, { briefs: { [NET_B]: { me_admin: true, can_invite: true } } });
  const { wrapper } = await mountPage(node, { hash: "settings" });
  return wrapper.get("#settings-leave-multi").text();
}

describe("with several networks", () => {
  it("leaving or deleting is in Device settings", async () => {
    expect(await pointer()).toBe("To leave or delete a network, open Networks → its tab → Device settings.");
  });
});
