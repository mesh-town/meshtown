/* The plate "this computer can't connect" on Home, for the network in `main`: it shows after 15 seconds of the refusal. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { membersReply, peerOneRow, selfRow } from "../testing/members-fixtures.mjs";
import { openMembersPage, stateWith } from "../testing/members-page.mjs";
import { unmountPage } from "../testing/page.mjs";
import { ADMISSION_REFUSED_DELAY_MS } from "../peers/peers-texts.ts";

beforeEach(() => vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] }));
afterEach(() => {
  unmountPage();
  vi.useRealTimers();
});

const them = { side: "them", reason: "no", at_unix: 1 };

describe("the plate on Home", () => {
  it("the plate is on Home for `main`, after 15 seconds", async () => {
    const page = await openMembersPage({ members: membersReply({ rows: [selfRow(), peerOneRow({ online: false, last_refusal: them })] }), state: stateWith({ admission_refused: true }), tab: "home" });
    expect(page.wrapper.find("#admission-refused").exists()).toBe(false);
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS);
    const plate = page.wrapper.get("#home-root #admission-refused");
    expect(plate.text()).toContain("peer-one refuses to let it in");
    expect(plate.text()).toContain("Ask peer-one to let this computer in.");
  });

  it("the flag is off and the list shows a refusal: no plate", async () => {
    const page = await openMembersPage({ members: membersReply({ rows: [selfRow(), peerOneRow({ online: false, last_refusal: them })] }), tab: "home" });
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS);
    expect(page.wrapper.find("#admission-refused").exists()).toBe(false);
  });

  it("the flag is on and nobody refuses by name: the plate has no names", async () => {
    const page = await openMembersPage({ members: membersReply({ rows: [selfRow(), peerOneRow({ online: false })] }), state: stateWith({ admission_refused: true }), tab: "home" });
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS);
    expect(page.wrapper.get("#home-root #admission-refused").text()).toContain("refuse to let it in, and no other member is online.");
  });
});
