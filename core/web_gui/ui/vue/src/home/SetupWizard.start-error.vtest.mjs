/* The setup after a failed start: the node keeps the saved network but is not running, so the browser and exit steps are hidden. */

import { nextTick } from "vue";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { baseState, mountSection } from "../testing/section.mjs";
import SetupWizard from "./SetupWizard.vue";
import { HOME_WIZARD_JOINED, HOME_WIZARD_JOINED_NOT_RUNNING } from "./home-card-texts.ts";

const MARK = "meshtown.homeSetup";
const WITH_PROXY = { socks: { enabled: true, listen: "127.0.0.1:1080", pac_url: "http://127.0.0.1:1080/proxy.pac" } };
const FAILED = { node: { ...baseState().node, start_error: "port 1091 for the browser proxy is in use" } };
let mounted;

beforeEach(() => localStorage.clear());
afterEach(() => mounted.wrapper.unmount());

describe("SetupWizard > failed start", () => {
  it("wizard_step2_shows_browser_card_without_start_error", () => {
    const { wrapper } = mountWizard(baseState({ ...WITH_PROXY, node: { ...baseState().node, start_error: null } }), "browser");
    expect(wrapper.find("#pac-row").exists()).toBe(true);
    expect(wrapper.text()).toContain(HOME_WIZARD_JOINED);
  });

  it("wizard_start_error_hides_browser_card_and_pac", () => {
    const { wrapper } = mountWizard(baseState({ ...WITH_PROXY, ...FAILED }), "browser");
    expect(wrapper.find("#pac-row").exists()).toBe(false);
    expect(wrapper.text()).not.toContain("proxy.pac");
    expect(wrapper.text()).toContain(HOME_WIZARD_JOINED_NOT_RUNNING);
    expect(wrapper.text()).not.toContain(HOME_WIZARD_JOINED);
  });

  it("wizard_start_error_hides_exit_card", () => {
    const { wrapper } = mountWizard(baseState({ ...WITH_PROXY, ...FAILED }), "exit");
    expect(wrapper.find("#home-exit").exists()).toBe(false);
    expect(wrapper.find("#field-node-name").exists()).toBe(false);
    expect(wrapper.findAll("button").map((b) => b.text())).not.toContain("Done");
  });

  it("wizard_start_error_cleared_restores_cards", async () => {
    const { wrapper, store } = mountWizard(baseState({ ...WITH_PROXY, ...FAILED }), "browser");
    expect(wrapper.find("#pac-row").exists()).toBe(false);
    store.state.node.start_error = null;
    await nextTick();
    expect(wrapper.find("#pac-row").exists()).toBe(true);
    expect(wrapper.text()).toContain(HOME_WIZARD_JOINED);
  });

  it("wizard_no_net_ignores_start_error", () => {
    const { wrapper } = mountWizard(baseState({ network: null, peers: [], ...FAILED }));
    expect(wrapper.find("#home-start-choice").exists()).toBe(true);
    expect(wrapper.text()).not.toContain(HOME_WIZARD_JOINED_NOT_RUNNING);
  });
});

function mountWizard(state, mark) {
  if (mark) localStorage.setItem(MARK, mark);
  return (mounted = mountSection(SetupWizard, { state }));
}

describe("SetupWizard > failed start heading", () => {
  it("wizard_start_error_step_is_not_marked_done", () => {
    const failed = { node: { ...baseState().node, start_error: "port 1091 for the browser proxy is in use" } };
    const { wrapper } = mountWizard(baseState({ ...WITH_PROXY, ...failed }), "browser");
    const step = wrapper.get("h3").element.closest(".wizard-step");
    expect(step.classList.contains("done")).toBe(false);
    expect(step.classList.contains("failed")).toBe(true);
  });

  it("wizard_joined_step_stays_done", () => {
    const { wrapper } = mountWizard(baseState({ ...WITH_PROXY, node: { ...baseState().node, start_error: null } }), "browser");
    expect(wrapper.get("h3").element.closest(".wizard-step").classList.contains("done")).toBe(true);
  });
});
