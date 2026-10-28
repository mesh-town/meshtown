/* The plate "this computer can't connect" survives a switch of the Home sub-tab or of the section: its 15-second count goes on. */

import { readFileSync } from "node:fs";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { openMembersPage, stateWith } from "../testing/members-page.mjs";
import { openTab, polls, unmountPage } from "../testing/page.mjs";
import { flushPromises } from "@vue/test-utils";
import { ADMISSION_REFUSED_DELAY_MS } from "../peers/peers-texts.ts";
import { baseConfig, baseState, mountSection } from "../testing/section.mjs";
import { MAC_CHROME, pressedTiles } from "../testing/wizard.mjs";
import HomeSection from "./HomeSection.vue";

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
  beforeEach(() => vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] }));
  afterEach(() => {
    unmountPage();
    vi.useRealTimers();
  });

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

describe("answers survive", () => {
  const PROXY_ON = { socks: { enabled: true, listen: "127.0.0.1:1080", pac_url: "http://127.0.0.1:1080/proxy.pac" } };
  let home;

  beforeEach(() => {
    localStorage.clear();
    localStorage.setItem("meshtown.homeSetup", "done");
    Object.defineProperty(navigator, "userAgent", { value: MAC_CHROME, configurable: true });
  });
  afterEach(() => {
    home?.wrapper.unmount();
    home = null;
    Reflect.deleteProperty(navigator, "userAgent");
  });

  const mountHome = () => (home = mountSection(HomeSection, { config: baseConfig(), state: baseState(PROXY_ON) }));
  const click = async (selector) => {
    await home.wrapper.get(selector).trigger("click");
    await flushPromises();
  };
  const rowIds = () => home.wrapper.findAll(".wizard-row").map((r) => r.attributes("data-step"));

  it("Firefox picked in the one-browser mode is still pressed after Main and back to Setup, on the instruction", async () => {
    mountHome();
    await click("#home-tab-wizard");
    await click("#scope-browser");
    await click("#browser-firefox");
    expect(pressedTiles(home.wrapper, "browser-seg")).toEqual(["firefox"]);
    await click("#home-tab-main");
    await click("#home-tab-wizard");
    expect(pressedTiles(home.wrapper, "browser-seg")).toEqual(["firefox"]);
    expect(pressedTiles(home.wrapper, "scope-seg")).toEqual(["browser"]);
    expect(rowIds().at(-1)).toBe("guide");
  });

  it("the browser picked survives a reload of the page: the same storage, a new section", async () => {
    mountHome();
    await click("#home-tab-wizard");
    await click("#scope-browser");
    await click("#browser-firefox");
    home.wrapper.unmount();
    mountHome();
    await click("#home-tab-wizard");
    expect(pressedTiles(home.wrapper, "browser-seg")).toEqual(["firefox"]);
    expect(rowIds().at(-1)).toBe("guide");
  });

  it("the system picked survives a reload as well, the detected one is not written", async () => {
    mountHome();
    await click("#home-tab-wizard");
    expect(localStorage.getItem("meshtown.proxyOs")).toBeNull();
    await click("#scope-system");
    await click("#os-windows");
    home.wrapper.unmount();
    mountHome();
    await click("#home-tab-wizard");
    expect(pressedTiles(home.wrapper, "os-seg")).toEqual(["windows"]);
  });
});
