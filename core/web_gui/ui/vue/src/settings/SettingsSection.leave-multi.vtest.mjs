/* The pointer text in Settings with several networks: it mentions deleting only when one of the networks is Managed. */

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { createFakeNode } from "../testing/fake-node.mjs";
import { managedMembers, managedState, nodeConfig, serveManaged } from "../testing/managed-fixtures.mjs";
import { NET_B, twoNets } from "../testing/overview-fixtures.mjs";
import { mountPage, unmountPage } from "../testing/page.mjs";

beforeEach(() => localStorage.clear());
afterEach(() => unmountPage());

async function pointer(managed) {
  const state = managedState({ network: { id_hex: "net-bbbb" } });
  const opts = managed ? twoNets({ label: "work", state, members: managedMembers(state), id_hex: "net-bbbb" }) : twoNets();
  const node = createFakeNode({ ...opts, config: nodeConfig() });
  serveManaged(node, managed ? { briefs: { [NET_B]: { kind: "Managed", me_admin: true, can_invite: true } } } : {});
  const { wrapper } = await mountPage(node, { hash: "settings" });
  return wrapper.get("#settings-leave-multi").text();
}

describe("with several networks", () => {
  it("one of them Managed: leaving or deleting is on the Members tab", async () => {
    expect(await pointer(true)).toBe("To leave or delete a network, open Members → its tab.");
  });

  it("all of them Peer: the former text", async () => {
    expect(await pointer(false)).toBe("To leave a network, open Members → its tab.");
  });
});
