/* "Connect the browser to Mesh Town", scope "one browser": the Firefox | Other choice, its default and memory. */

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { FIREFOX_STEPS, SCOPE_SYSTEM } from "../common/pac-texts.ts";
import { createStore, STORE_KEY } from "../core/store.ts";
import { applySnapshot } from "../testing/snapshot.mjs";
import { baseConfig, baseState } from "../testing/section.mjs";
import BrowserCard from "./BrowserCard.vue";

const PROXY_ON = { socks: { enabled: true, listen: "127.0.0.1:1080", pac_url: "http://127.0.0.1:1080/proxy.pac" } };
let mounted;

beforeEach(() => { localStorage.clear(); localStorage.setItem("meshtown.proxyScope", "browser"); });
afterEach(() => mounted.unmount());

function mountCard() {
  const store = createStore();
  applySnapshot(store, { state: baseState(PROXY_ON), config: baseConfig(), configRev: 1 });
  return (mounted = mount(BrowserCard, { attachTo: document.body, global: { provide: { [STORE_KEY]: store } } }));
}

const pressed = (w) => ["firefox", "other"].filter((id) => w.get("#browser-" + id).attributes("aria-pressed") === "true");

describe("BrowserCard > browser choice", () => {
  it("the segment is hidden for the system scope and without an answer", async () => {
    localStorage.clear();
    const w = mountCard();
    expect(w.find("#browser-seg").exists()).toBe(false);
    await w.get("#scope-system").trigger("click");
    expect(w.find("#browser-seg").exists()).toBe(false);
    await w.get("#scope-browser").trigger("click");
    expect(w.find("#browser-seg").attributes("role")).toBe("group");
  });

  it("a saved firefox shows the Firefox steps", () => {
    localStorage.setItem("meshtown.proxyBrowser", "firefox");
    const w = mountCard();
    expect(pressed(w)).toEqual(["firefox"]);
    expect(w.findAll("#firefox-steps li").map((li) => li.text())).toEqual(FIREFOX_STEPS);
    expect(w.find("#browser-other-steps").exists()).toBe(false);
  });

  it("a saved chrome shows the Other block", () => {
    localStorage.setItem("meshtown.proxyBrowser", "chrome");
    const w = mountCard();
    expect(pressed(w)).toEqual(["other"]);
    expect(w.find("#firefox-steps").exists()).toBe(false);
    expect(w.get("#browser-other-steps").text()).toContain("proxy extension");
  });

  it("clicks switch the blocks and are remembered", async () => {
    localStorage.setItem("meshtown.proxyBrowser", "firefox");
    const w = mountCard();
    await w.get("#browser-other").trigger("click");
    expect(localStorage.getItem("meshtown.proxyBrowser")).toBe("other");
    expect(w.find("#browser-other-steps").exists()).toBe(true);
    await w.get("#browser-firefox").trigger("click");
    expect(localStorage.getItem("meshtown.proxyBrowser")).toBe("firefox");
    expect(w.find("#firefox-steps").exists()).toBe(true);
  });

  it("the link in the Other block switches the scope to the system", async () => {
    localStorage.setItem("meshtown.proxyBrowser", "other");
    const w = mountCard();
    const link = w.get("#browser-other-steps .link-more");
    expect(link.text()).toBe(SCOPE_SYSTEM);
    await link.trigger("click");
    expect(localStorage.getItem("meshtown.proxyScope")).toBe("system");
    expect(w.find("#os-seg").exists()).toBe(true);
    expect(w.find("#browser-seg").exists()).toBe(false);
  });
});
