/* The header line on the whole page when the members turned this computer down: after the hold delay it turns red
 * ("Sites won't open · …"), and it is green again once an online neighbour shows up. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { membersReply, peerOneRow, selfRow } from "../testing/members-fixtures.mjs";
import { openMembersPage, stateWith } from "../testing/members-page.mjs";
import { polls, unmountPage } from "../testing/page.mjs";
import { ADMISSION_REFUSED_DELAY_MS } from "../peers/peers-texts.ts";

beforeEach(() => vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] }));
afterEach(() => {
  unmountPage();
  vi.useRealTimers();
});

const them = { side: "them", reason: "no", at_unix: 1 };
const refused = () => membersReply({ rows: [selfRow(), peerOneRow({ online: false, last_refusal: them })] });
const flagged = (on) => stateWith({ admission_refused: on });
const open = (on = true) => openMembersPage({ members: refused(), state: flagged(on), tab: "home" });

const line = (page) => page.wrapper.get("#status-line");

describe("the header line with a held admission refusal", () => {
  it("before the delay: the usual green \"Sites open directly\" line", async () => {
    const page = await open();
    expect(line(page).text()).toMatch(/^Sites open directly/);
    expect(page.wrapper.find(".dot-red").exists()).toBe(false);
  });

  it("after the delay: red, starting with \"Sites won't open\"", async () => {
    const page = await open();
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS);
    expect(line(page).text()).toMatch(/^Sites won't open/);
    expect(page.wrapper.find(".dot-red").exists()).toBe(true);
  });

  it("the flag is off though the list shows a refusal: green", async () => {
    const page = await open(false);
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS);
    expect(line(page).text()).toMatch(/^Sites open directly/);
  });

  it("the server drops the flag: green again", async () => {
    const page = await open();
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS);
    expect(line(page).text()).toMatch(/^Sites won't open/);
    page.node.state = flagged(false);
    await polls(page.node, 1);
    expect(line(page).text()).toMatch(/^Sites open directly/);
    expect(page.wrapper.find(".dot-red").exists()).toBe(false);
    expect(page.wrapper.find(".dot-green").exists()).toBe(true);
  });
});
