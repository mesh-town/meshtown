/* "Connect the browser to Mesh Town", the scope question: nothing shown but the question and the address until
 * the answer; "system" shows the system steps, "browser" shows the Firefox steps; the answer is remembered. */

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { defineComponent, h, provide } from "vue";
import { BROWSER_SCOPE_NOTE, FIREFOX_STEPS, SCOPE_BROWSER, SCOPE_QUESTION, SCOPE_SYSTEM } from "../common/pac-texts.ts";
import { createStore, STORE_KEY } from "../core/store.ts";
import { applySnapshot } from "../testing/snapshot.mjs";
import { baseConfig, baseState } from "../testing/section.mjs";
import BrowserCard from "./BrowserCard.vue";
import SetupWizard from "./SetupWizard.vue";
import { HOME_KEY, useHome } from "./use-home.ts";

const PROXY_ON = { socks: { enabled: true, listen: "127.0.0.1:1080", pac_url: "http://127.0.0.1:1080/proxy.pac" } };
const KEY = "meshtown.proxyScope";
let mounted;

beforeEach(() => localStorage.clear());
afterEach(() => mounted.unmount());

function mountCard(state = baseState(PROXY_ON)) {
  const store = createStore();
  applySnapshot(store, { state, config: baseConfig(), configRev: 1 });
  return (mounted = mount(BrowserCard, { attachTo: document.body, global: { provide: { [STORE_KEY]: store } } }));
}

const pressed = (wrapper) => ["system", "browser"].filter((id) => wrapper.get("#scope-" + id).attributes("aria-pressed") === "true");

describe("BrowserCard > scope question", () => {
  it("no answer: the question, two buttons, the address; no steps of either kind, no manual link", () => {
    const wrapper = mountCard();
    expect(wrapper.get("#scope-q").text()).toBe(SCOPE_QUESTION);
    expect(wrapper.get("#scope-system").text()).toBe(SCOPE_SYSTEM);
    expect(wrapper.get("#scope-browser").text()).toBe(SCOPE_BROWSER);
    expect(wrapper.get("#scope-system").text()).toBe("All apps on this computer (recommended)");
    expect(wrapper.get("#scope-browser").text()).toBe("Only in one browser");
    expect(pressed(wrapper)).toEqual([]);
    expect(wrapper.get("#pac-url-text").text()).toBe("http://127.0.0.1:1080/proxy.pac");
    for (const id of ["#pac-steps", "#os-seg", "#firefox-steps", "#browser-scope-note", "#manual-toggle", "#proxy-setup"]) {
      expect(wrapper.find(id).exists(), id).toBe(false);
    }
  });

  it("proxy off: no question", () => {
    const wrapper = mountCard(baseState({ socks: { enabled: false, listen: null, pac_url: null } }));
    expect(wrapper.find("#scope-seg").exists()).toBe(false);
  });

  it("system: the system steps and the manual link; no Firefox steps", async () => {
    const wrapper = mountCard();
    await wrapper.get("#scope-system").trigger("click");
    expect(pressed(wrapper)).toEqual(["system"]);
    expect(wrapper.find("#os-seg").exists()).toBe(true);
    expect(wrapper.find("#manual-toggle").exists()).toBe(true);
    expect(wrapper.find("#firefox-steps").exists()).toBe(false);
  });

  it("browser: the Firefox steps, the note about other browsers and the manual link; no system steps", async () => {
    const wrapper = mountCard();
    await wrapper.get("#scope-browser").trigger("click");
    expect(pressed(wrapper)).toEqual(["browser"]);
    expect(wrapper.findAll("#firefox-steps li").map((li) => li.text())).toEqual(FIREFOX_STEPS);
    expect(wrapper.get("#browser-scope-note").text()).toBe(BROWSER_SCOPE_NOTE);
    expect(wrapper.find("#manual-toggle").exists()).toBe(true);
    for (const id of ["#pac-steps", "#os-seg"]) expect(wrapper.find(id).exists(), id).toBe(false);
  });

  it("a click on the other button switches the view", async () => {
    const wrapper = mountCard();
    await wrapper.get("#scope-browser").trigger("click");
    await wrapper.get("#scope-system").trigger("click");
    expect(pressed(wrapper)).toEqual(["system"]);
    expect(wrapper.find("#firefox-steps").exists()).toBe(false);
    expect(wrapper.find("#os-seg").exists()).toBe(true);
  });

  it("the manual block opens in the browser view too", async () => {
    const wrapper = mountCard();
    await wrapper.get("#scope-browser").trigger("click");
    await wrapper.get("#manual-toggle").trigger("click");
    expect(wrapper.find("#proxy-setup").exists()).toBe(true);
  });

  it("the answer is remembered and survives remounting", async () => {
    const first = mountCard();
    await first.get("#scope-browser").trigger("click");
    expect(localStorage.getItem(KEY)).toBe("browser");
    first.unmount();
    const second = mountCard();
    expect(pressed(second)).toEqual(["browser"]);
    expect(second.find("#firefox-steps").exists()).toBe(true);
  });

  it("garbage in the storage counts as no answer", () => {
    localStorage.setItem(KEY, "both");
    const wrapper = mountCard();
    expect(pressed(wrapper)).toEqual([]);
    expect(wrapper.find("#firefox-steps").exists()).toBe(false);
  });

  it("the scope picked on the card is the one the wizard shows: one answer for both", async () => {
    const store = createStore();
    applySnapshot(store, { state: baseState(PROXY_ON), config: baseConfig(), configRev: 1 });
    const Both = defineComponent({
      setup() {
        provide(HOME_KEY, useHome(store));
        return () => [h(BrowserCard), h(SetupWizard)];
      },
    });
    mounted = mount(Both, { attachTo: document.body, global: { provide: { [STORE_KEY]: store } } });
    await mounted.get("#home-proxy #scope-browser").trigger("click");
    expect(mounted.get("#wizard-screen #scope-browser").attributes("aria-pressed")).toBe("true");
    expect(mounted.get("#wizard-screen #scope-system").attributes("aria-pressed")).toBe("false");
  });
});
