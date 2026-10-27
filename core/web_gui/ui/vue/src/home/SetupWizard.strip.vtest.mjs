/* The strip of steps above the wizard screen: which steps are buttons, what a click on them does, and Back with its arrow. */

import { afterEach, describe, expect, it } from "vitest";
import { click, mountWizard, step, unmountWizard } from "../testing/wizard.mjs";

let mounted;
const mount = (opts) => (mounted = mountWizard({ mark: "browser", scope: "system", ...opts }));

afterEach(() => unmountWizard(mounted));

const names = (wrapper) => wrapper.findAll("#wizard-strip li").map((li) => li.text().replace(/\s+/g, " "));

describe("SetupWizard > strip", () => {
  it("shows the named steps with the current one marked", () => {
    const { wrapper } = mount();
    expect(wrapper.get("#wizard-strip").attributes("aria-label")).toBe("Setup steps");
    expect(names(wrapper)).toEqual(["✓Join", "2Apps", "3System", "4Browser", "5Set up", "6Exit"]);
    expect(wrapper.get("#wizard-strip-scope").attributes("aria-current")).toBe("step");
  });

  it("joined: Join is not a button; locked steps are not buttons", () => {
    const { wrapper } = mount();
    expect(wrapper.get("#wizard-strip-join").element.tagName).toBe("SPAN");
    expect(wrapper.get("#wizard-strip-os").element.tagName).toBe("SPAN");
    expect(wrapper.get("#wizard-strip-scope").element.tagName).toBe("SPAN");
  });

  it("a click on a passed step returns to it, and the step right of it is open", async () => {
    const { wrapper } = mount();
    await click(wrapper, "#wizard-next");
    expect(step(wrapper)).toBe("os");
    await click(wrapper, "#wizard-strip-scope");
    expect(step(wrapper)).toBe("scope");
    expect(wrapper.get("#wizard-strip-os").element.tagName).toBe("BUTTON");
  });

  it("a click on an open step goes forward to it", async () => {
    const { wrapper } = mount();
    await click(wrapper, "#wizard-next");
    await click(wrapper, "#wizard-back");
    await click(wrapper, "#wizard-strip-os");
    expect(step(wrapper)).toBe("os");
  });

  it("Back keeps its accessible name and shows an arrow", () => {
    const { wrapper } = mount({ mark: "exit" });
    const back = wrapper.get("#wizard-back");
    expect(back.text()).toBe("← Back");
    expect(back.get("span").attributes("aria-hidden")).toBe("true");
  });

  it("the strip is gone when the start failed, and the progress line stays for readers", () => {
    const { wrapper } = mount({ startError: true });
    expect(wrapper.find("#wizard-strip").exists()).toBe(false);
    expect(mount().wrapper.get("#wizard-progress").text()).toBe("Step 2 of 6");
  });
});
