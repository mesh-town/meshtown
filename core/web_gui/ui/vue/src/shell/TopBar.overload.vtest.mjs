/* The header line shows "⚠️ Turning members away" on any tab while this computer's exit is overloaded. */

import { afterEach, describe, expect, it } from "vitest";
import { openMembersPage, stateWith } from "../testing/members-page.mjs";
import { membersReply, pendingRow, selfRow } from "../testing/members-fixtures.mjs";
import { unmountPage } from "../testing/page.mjs";

afterEach(() => unmountPage());

const load = (overloaded) => ({ streams: 1, limit: 1, limit_auto: 1, overloaded, admitted: 0, refused: 0, members: [], elsewhere_refused: 0 });
const overloadedState = (overloaded) => {
  const state = stateWith();
  return { ...state, exit: { ...state.exit, load: load(overloaded) } };
};

describe("the header overload link", () => {
  it("link shown on the home tab while overloaded", async () => {
    const page = await openMembersPage({ state: overloadedState("limit"), tab: "home" });
    const link = page.wrapper.get("#strip-overload");
    expect(link.text()).toBe("⚠️ Turning members away");
    expect(link.attributes("href")).toMatch(/^#peers(\/|$)/);
  });

  it("no link without an overload", async () => {
    const page = await openMembersPage({ state: overloadedState(null), tab: "home" });
    expect(page.wrapper.find("#strip-overload").exists()).toBe(false);
  });

  it("link sits next to the join-requests link", async () => {
    const members = membersReply({ rows: [selfRow(), pendingRow()], me_admin: true });
    const page = await openMembersPage({ members, state: overloadedState("limit"), tab: "home" });
    const strip = page.wrapper.get("#status-line").element;
    const link = strip.querySelector("#strip-overload");
    expect(link.nextElementSibling.id).toBe("strip-vote");
    expect(link.previousElementSibling.className).toBe("status-text");
  });
});
