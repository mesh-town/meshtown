/* `managed-ctx.ts`: the facts the member menu and the leave dialog read. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { canDeleteNet, managedFacts } from "./managed-ctx.ts";
import { ANNA_FULL, ME_FULL, managedState } from "../testing/managed-fixtures.mjs";

const facts = (meRole, admins) => {
  const net = managedState({ meRole, admins }).network;
  return managedFacts(net, null, ME_FULL);
};

describe("managedFacts", () => {
  it("onlyAdmin is true for 1 admin, false for 0 and 2", () => {
    assert.equal(facts("admin", [ME_FULL]).onlyAdmin, true);
    assert.equal(facts("member", []).onlyAdmin, false);
    assert.equal(facts("admin", [ME_FULL, ANNA_FULL]).onlyAdmin, false);
  });

  it("reads the role and the admins, and has no `managed` flag", () => {
    const got = facts("admin", [ME_FULL, ANNA_FULL]);
    assert.equal(got.meAdmin, true);
    assert.deepEqual(got.adminIds, [ME_FULL, ANNA_FULL]);
    assert.equal("managed" in got, false);
  });

  it("the member list wins over the snapshot", () => {
    const net = managedState({ meRole: "member", admins: [ANNA_FULL] }).network;
    const got = managedFacts(net, { admins: [ME_FULL], me_admin: true }, ME_FULL);
    assert.equal(got.meAdmin, true);
    assert.deepEqual(got.adminIds, [ME_FULL]);
  });
});

describe("canDeleteNet", () => {
  it("follows can_delete_network, or the admin role when it is absent", () => {
    assert.equal(canDeleteNet({ can_delete_network: false, me_role: "admin" }), false);
    assert.equal(canDeleteNet({ me_role: "admin" }), true);
    assert.equal(canDeleteNet({ me_role: "member" }), false);
    assert.equal(canDeleteNet(null), false);
  });
});
