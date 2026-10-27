/* The plate "this computer can't connect" survives a switch of the Home sub-tab or of the section: its 15-second count goes on. */

import { readFileSync } from "node:fs";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { openMembersPage, stateWith } from "../testing/members-page.mjs";
import { openTab, polls, unmountPage } from "../testing/page.mjs";
import { ADMISSION_REFUSED_DELAY_MS } from "../peers/peers-texts.ts";

beforeEach(() => vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] }));
afterEach(() => {
  unmountPage();
  vi.useRealTimers();
});

const read = (name) => JSON.parse(readFileSync(new URL("../testing/" + name, import.meta.url), "utf8"));
const MEMBERS = read("refused-node.members.json");
const SUMMARY = read("refused-node.state-members.json");

const plates = (page) => page.wrapper.findAll("#admission-refused");
const openRefused = (tab = "home") => openMembersPage({ members: MEMBERS, state: stateWith(SUMMARY), tab });
const pick = (page, tab) => page.wrapper.get("#home-tab-" + tab).trigger("click");

// The flag is changed the way it changes live: the node answers differently and the next poll brings it to the page.
async function setFlag(page, refused) {
  page.node.state = stateWith({ ...SUMMARY, admission_refused: refused });
  await polls(page.node, 1);
}

describe("the plate while switching tabs", () => {
  it("keeps the plate when switching Overview to Setup", async () => {
    const page = await openRefused();
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS);
    expect(plates(page)).toHaveLength(1);
    await pick(page, "wizard");
    expect(plates(page)).toHaveLength(1);
  });

  it("keeps the plate when switching Setup to Overview", async () => {
    const page = await openRefused();
    await pick(page, "wizard");
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS);
    expect(plates(page)).toHaveLength(1);
    await pick(page, "main");
    expect(plates(page)).toHaveLength(1);
  });

  it("keeps the plate across sections", async () => {
    const page = await openRefused();
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS);
    await openTab(page.wrapper, "peers");
    expect(plates(page)).toHaveLength(1);
    expect(page.wrapper.find("#sec-peers #admission-refused").exists()).toBe(true);
    await openTab(page.wrapper, "home");
    expect(plates(page)).toHaveLength(1);
    expect(page.wrapper.find("#sec-home #admission-refused").exists()).toBe(true);
  });

  it("drops the plate when the refusal is lifted and waits again", async () => {
    const page = await openRefused();
    await pick(page, "wizard");
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS);
    expect(plates(page)).toHaveLength(1);
    await setFlag(page, false);
    expect(plates(page)).toHaveLength(0);
    await setFlag(page, true);
    expect(plates(page)).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(ADMISSION_REFUSED_DELAY_MS);
    expect(plates(page)).toHaveLength(1);
  });
});
