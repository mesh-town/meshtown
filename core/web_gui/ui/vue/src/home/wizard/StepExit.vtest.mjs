/* The wizard exit screen: the exit list, this computer's own switch (not inside the app), and a warning while
 * blocked sites have no exit. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { baseConfig, baseState, mountSection, PEER_FULL } from "../../testing/section.mjs";
import StepExit from "./StepExit.vue";

vi.mock("../../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const COMPUTER_TITLE = "This computer";
const WARNING = "Blocked sites won't open until you choose a member above.";
let mounted;

beforeEach(() => localStorage.clear());
afterEach(() => mounted.wrapper.unmount());

describe("StepExit", () => {
  describe("in a browser", () => {
    it("shows the list of exits, the \"This computer\" subtitle and the computer card", () => {
      const { wrapper } = setup({ inApp: false });
      expect(wrapper.find("#home-exit").exists(), "the exit list").toBe(true);
      expect(wrapper.findAll("h4").map((h) => h.text())).toContain(COMPUTER_TITLE);
      expect(wrapper.find("#home-name").exists(), "the computer name field").toBe(true);
      expect(wrapper.find("#home-exit-server").exists(), "the \"allow others\" switch").toBe(true);
    });
  });

  describe("in the app", () => {
    it("shows the list of exits but neither the subtitle nor the computer card", () => {
      const { wrapper } = setup({ inApp: true });
      expect(wrapper.find("#home-exit").exists(), "the exit list").toBe(true);
      expect(wrapper.text()).not.toContain(COMPUTER_TITLE);
      expect(wrapper.find("#home-name").exists()).toBe(false);
      expect(wrapper.find("#home-exit-server").exists()).toBe(false);
    });
  });

  describe("no exit is chosen (\"As usual\")", () => {
    it("warns that blocked sites will not open", () => {
      const { wrapper } = setup({ inApp: false });
      expect(wrapper.get("#exit-direct").element.checked, "the precondition: directly").toBe(true);
      expect(wrapper.get("#exit-direct-warn").text()).toBe(WARNING);
    });

    it("warns in the app too", () => {
      const { wrapper } = setup({ inApp: true });
      expect(wrapper.get("#exit-direct-warn").text()).toBe(WARNING);
    });
  });

  describe("a member is chosen as the exit", () => {
    it("has no warning", () => {
      const { wrapper } = setup({ inApp: false, exit: PEER_FULL });
      expect(wrapper.get(`input[data-id="${PEER_FULL}"]`).element.checked, "the precondition: the member is chosen").toBe(true);
      expect(wrapper.find("#exit-direct-warn").exists()).toBe(false);
    });
  });
});

// Mounts the screen; `exit` — the id of the member the node already routes through (none by default).
function setup({ inApp, exit }) {
  const state = exit === undefined
    ? baseState()
    : baseState({ route: { state: "exit", exits: [{ id_full: exit, active: true }] }, config_rev: 1 });
  const config = baseConfig(exit === undefined ? {} : { route: { exits: [exit] } });
  return (mounted = mountSection(StepExit, { state, config, props: { inApp } }));
}
