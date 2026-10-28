/* The strip of steps beside the wizard screen: which steps are buttons, what a click on them does, and Back with its arrow. */

import { afterEach, describe, expect, it } from "vitest";
import { click, cursor, mountWizard, step, unmountWizard } from "../testing/wizard.mjs";

let mounted;
const mount = (opts) => (mounted = mountWizard({ mark: "browser", scope: "system", ...opts }));

afterEach(() => unmountWizard(mounted));

const names = (wrapper) => wrapper.findAll("#wizard-strip li").map((li) => li.text().replace(/\s+/g, " "));

describe("SetupWizard > strip", () => {
  it("shows the named steps with the current one marked", () => {
    const { wrapper } = mount();
    expect(wrapper.get("#wizard-strip").attributes("aria-label")).toBe("Setup steps");
    expect(names(wrapper)).toEqual(["✓Join", "✓Apps", "✓System", "4Set up", "5Exit"]);
    expect(wrapper.get("#wizard-strip-guide").attributes("aria-current")).toBe("step");
  });

  it("joined: Join is not a button; locked steps are not buttons", () => {
    const { wrapper } = mount({ scope: null });
    expect(wrapper.get("#wizard-strip-join").element.tagName).toBe("SPAN");
    expect(wrapper.get("#wizard-strip-os").element.tagName).toBe("SPAN");
    expect(wrapper.get("#wizard-strip-scope").element.tagName).toBe("SPAN");
  });

  it("the rows already open are buttons, the instruction under the cursor is not", () => {
    const { wrapper } = mount();
    expect(wrapper.get("#wizard-strip-os").element.tagName).toBe("BUTTON");
    expect(wrapper.get("#wizard-strip-guide").element.tagName).toBe("SPAN");
  });

  it("a click on a row of the page puts the focus on it and keeps the page", async () => {
    const { wrapper } = mount();
    await click(wrapper, "#wizard-strip-os");
    expect(step(wrapper)).toBe("choices");
    expect(document.activeElement).toBe(wrapper.get("#wizard-q-os").element);
  });

  it("a click on a row from the exit screen returns to the page with the rows open", async () => {
    const { wrapper } = mount({ mark: "exit" });
    expect(step(wrapper)).toBe("exit");
    await click(wrapper, "#wizard-strip-os");
    expect(step(wrapper)).toBe("choices");
    expect(cursor(wrapper)).toBe("guide");
    expect(document.activeElement).toBe(wrapper.get("#wizard-q-os").element);
  });

  it("a click on an open step goes forward to it", async () => {
    const { wrapper } = mount();
    await click(wrapper, "#wizard-next");
    await click(wrapper, "#wizard-back");
    expect(wrapper.get("#wizard-strip-exit").element.tagName).toBe("BUTTON");
    await click(wrapper, "#wizard-strip-exit");
    expect(step(wrapper)).toBe("exit");
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
    expect(mount().wrapper.get("#wizard-progress").text()).toBe("Step 4 of 5");
  });
});
