/* The setup after a failed start: the node keeps the saved network but is not running, so the wizard shows only the failed line
 * (no screen, no navigation) instead of questions that would point at nothing. */

import { nextTick } from "vue";
import { afterEach, describe, expect, it } from "vitest";
import { mountWizard, unmountWizard } from "../testing/wizard.mjs";
import { HOME_WIZARD_JOINED, HOME_WIZARD_JOINED_NOT_RUNNING } from "./home-card-texts.ts";

let mounted;
const mount = (opts) => (mounted = mountWizard(opts));

afterEach(() => unmountWizard(mounted));

describe("SetupWizard > failed start", () => {
  it("failed start shows only the failed line", () => {
    const { wrapper } = mount({ mark: "browser", startError: true });
    expect(wrapper.get(".wizard-step.failed h3").text()).toBe(HOME_WIZARD_JOINED_NOT_RUNNING);
    expect(wrapper.find("#wizard-screen").exists()).toBe(false);
    expect(wrapper.find("#wizard-q").exists()).toBe(false);
    expect(wrapper.find(".wizard-nav").exists()).toBe(false);
    expect(wrapper.find("#wizard-joined").exists()).toBe(false);
    expect(wrapper.text()).not.toContain(HOME_WIZARD_JOINED);
  });

  it("failed start leaves no address and no setup steps", () => {
    const { wrapper } = mount({ mark: "browser", scope: "system", startError: true });
    expect(wrapper.find("#pac-row").exists()).toBe(false);
    expect(wrapper.find("#pac-url-text").exists()).toBe(false);
    expect(wrapper.text()).not.toContain("proxy.pac");
  });

  it("failed start on the exit mark leaves no exit list, name field or buttons", () => {
    const { wrapper } = mount({ mark: "exit", startError: true });
    expect(wrapper.find("#home-exit").exists()).toBe(false);
    expect(wrapper.find("#field-node-name").exists()).toBe(false);
    expect(wrapper.findAll("button").map((b) => b.text())).not.toContain("Finish");
  });

  it("the failed line is not marked done", () => {
    const { wrapper } = mount({ mark: "browser", startError: true });
    const step = wrapper.get(".wizard-step.failed");
    expect(step.classes()).not.toContain("done");
  });

  it("without a start error the screen is there and the joined line is not the failed one", () => {
    const { wrapper } = mount({ mark: "browser" });
    expect(wrapper.find("#wizard-screen").exists()).toBe(true);
    expect(wrapper.find(".wizard-step.failed").exists()).toBe(false);
    expect(wrapper.get("#wizard-joined").text()).toContain(HOME_WIZARD_JOINED);
  });

  it("a cleared start error brings the screen back", async () => {
    const { wrapper, store } = mount({ mark: "browser", startError: true });
    expect(wrapper.find("#wizard-screen").exists()).toBe(false);
    store.state.node.start_error = null;
    await nextTick();
    expect(wrapper.find("#wizard-screen").exists()).toBe(true);
    expect(wrapper.find(".wizard-step.failed").exists()).toBe(false);
  });

  it("without a network a start error is ignored: the join screen stays", () => {
    const { wrapper } = mount({ hasNet: false, startError: true });
    expect(wrapper.find("#home-start-choice").exists()).toBe(true);
    expect(wrapper.text()).not.toContain(HOME_WIZARD_JOINED_NOT_RUNNING);
  });
});
