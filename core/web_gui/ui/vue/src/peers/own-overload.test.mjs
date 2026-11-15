/* `own-overload.ts`: where the header "turning members away" link leads. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ownOverloadTarget } from "./own-overload.ts";
import { createStore } from "../core/store.ts";
import { baseState } from "../testing/section.mjs";
import { membersReply, selfRow } from "../testing/members-fixtures.mjs";

const A = "main";
const B = "k7q2xa";
const load = (overloaded) => ({ exit: { ...baseState().exit, load: overloaded === undefined ? null : { streams: 1, limit: 1, limit_auto: 1, overloaded, admitted: 0, refused: 0, members: [], elsewhere_refused: 0 } } });

describe("ownOverloadTarget", () => {
  it("null without an overload", () => {
    assert.equal(ownOverloadTarget(setup({ [A]: load(null) })), null);
  });
  it("the first visible network with an overload", () => {
    assert.equal(ownOverloadTarget(setup({ [A]: load(null), [B]: load("limit") })), `#peers/${B}`);
  });
  it("skips a network where this computer is excluded", () => {
    assert.equal(ownOverloadTarget(setup({ [A]: { ...load("limit"), excluded: true }, [B]: load("share") })), `#peers/${B}`);
  });
  it("null when exit.load is null", () => {
    assert.equal(ownOverloadTarget(setup({ [A]: load(undefined) })), null);
  });
});

function setup(specs) {
  const store = createStore();
  for (const [id, spec] of Object.entries(specs)) {
    const b = { id, label: id, id_hex: "00" + id, status: "running", exit_enabled: true, proxy: id === A, route_state: "direct", exits: [], joining: false, members_admitting: 0, me_excluded: false, me_pending: false, peers_online: 1, peers_total: 1 };
    const state = baseState({ exit: spec.exit });
    const members = membersReply({ rows: [selfRow()], me_excluded: spec.excluded === true });
    store.nets.push(b);
    store.slots[id] = { id, brief: b, state, members, config: null, configRev: null, configMeta: null, bootId: null };
    if (id === store.lead) Object.assign(store, { state, members });
  }
  return store;
}
