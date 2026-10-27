/* The plate "this computer can't connect" is in the document once: on the active tab only (Overview or Members). */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { membersReply, peerOneRow, selfRow } from "../testing/members-fixtures.mjs";
import { openMembersPage, stateWith } from "../testing/members-page.mjs";
import { openTab, unmountPage } from "../testing/page.mjs";
import { ADMISSION_REFUSED_DELAY_MS } from "../peers/peers-texts.ts";

beforeEach(() => vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] }));
afterEach(() => {
  unmountPage();
  vi.useRealTimers();
});

const them = { side: "them", reason: "no", at_unix: 1 };
const refusing = () => membersReply({ rows: [selfRow(), peerOneRow({ online: false, last_refusal: them })] });
const plates = (page) => page.wrapper.findAll("#admission-refused");

describe("one plate in the document", () => {
  it("Overview active, refusal held for 15 seconds: one plate, in the visible Overview panel", async () => {
    const page = await openMembersPage({ members: refusing(), state: stateWith({ admission_refused: true }), tab: "home" });
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS);
    expect(plates(page)).toHaveLength(1);
    expect(page.wrapper.find("#sec-home #admission-refused").exists()).toBe(true);
    expect(page.wrapper.get("#sec-home").attributes("hidden")).toBeUndefined();
  });

  it("Members active, refusal held for 15 seconds: one plate, in the visible Members panel", async () => {
    const page = await openMembersPage({ members: refusing(), state: stateWith({ admission_refused: true }), tab: "peers" });
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS);
    expect(plates(page)).toHaveLength(1);
    expect(page.wrapper.find("#sec-peers #admission-refused").exists()).toBe(true);
    expect(page.wrapper.get("#sec-peers").attributes("hidden")).toBeUndefined();
  });

  it("switching the tab moves the single plate with it", async () => {
    const page = await openMembersPage({ members: refusing(), state: stateWith({ admission_refused: true }), tab: "home" });
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS);
    await openTab(page.wrapper, "peers");
    expect(plates(page)).toHaveLength(1);
    expect(page.wrapper.find("#sec-peers #admission-refused").exists()).toBe(true);
  });

  it("no refusal: no plate on either tab", async () => {
    const page = await openMembersPage({ members: refusing(), tab: "home" });
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS);
    expect(plates(page)).toHaveLength(0);
    await openTab(page.wrapper, "peers");
    expect(plates(page)).toHaveLength(0);
  });
});
