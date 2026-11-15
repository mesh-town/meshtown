/* The warning strip under the header shows on any tab while this computer's exit is overloaded. */

import { afterEach, describe, expect, it } from "vitest";
import { openMembersPage, stateWith } from "../testing/members-page.mjs";
import { unmountPage } from "../testing/page.mjs";

afterEach(() => unmountPage());

const load = (overloaded) => ({ streams: 1, limit: 1, limit_auto: 1, overloaded, admitted: 0, refused: 0, members: [], elsewhere_refused: 0 });
const overloadedState = (overloaded) => {
  const state = stateWith();
  return { ...state, exit: { ...state.exit, load: load(overloaded) } };
};

describe("the overload warning strip", () => {
  it("strip shown on the home tab while overloaded", async () => {
    const page = await openMembersPage({ state: overloadedState("limit"), tab: "home" });
    const strip = page.wrapper.get("#overload-strip");
    expect(strip.classes()).toContain("warn-strip");
    expect(strip.text().startsWith("⚠️ This computer is turning members away.")).toBe(true);
    expect(page.wrapper.get("#overload-strip-link").attributes("href")).toMatch(/^#peers(\/|$)/);
  });

  it("no strip without an overload", async () => {
    const page = await openMembersPage({ state: overloadedState(null), tab: "home" });
    expect(page.wrapper.find("#overload-strip").exists()).toBe(false);
  });

  it("the header line has no overload link", async () => {
    const page = await openMembersPage({ state: overloadedState("limit"), tab: "home" });
    const line = page.wrapper.get("#status-line");
    expect(line.find("#strip-overload").exists()).toBe(false);
    expect(line.text()).not.toContain("turning members away");
  });

  it("strip shown on another tab too", async () => {
    const page = await openMembersPage({ state: overloadedState("limit"), tab: "settings" });
    expect(page.wrapper.find("#overload-strip").exists()).toBe(true);
  });
});
