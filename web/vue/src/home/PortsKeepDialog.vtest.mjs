/* The question about ports when changing the network: the answer "yes" | "no" | "cancel"; no showModal — "no". */

import { afterEach, beforeAll, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import PortsKeepDialog from "./PortsKeepDialog.vue";

let wrapper;

// happy-dom doesn't implement <dialog>: its own showModal/close model with a close event.
beforeAll(() => {
  const proto = HTMLDialogElement.prototype;
  proto.showModal = function showModal() { this.setAttribute("open", ""); };
  proto.close = function close(value) {
    if (value !== undefined) this.returnValue = value;
    this.removeAttribute("open");
    this.dispatchEvent(new Event("close"));
  };
});
afterEach(() => wrapper.unmount());

describe("PortsKeepDialog", () => {
  it("\"Yes\" answers yes", async () => {
    const { ask } = setup();
    const answer = ask();
    await wrapper.get("#ports-keep-yes").trigger("click");
    expect(await answer).toBe("yes");
  });

  it("\"No\" answers no, and the focus on opening is on \"No\"", async () => {
    const { ask } = setup();
    const answer = ask();
    expect(document.activeElement).toBe(wrapper.get("#ports-keep-no").element);
    await wrapper.get("#ports-keep-no").trigger("click");
    expect(await answer).toBe("no");
  });

  it("closing without a choice (Escape) answers cancel, not the previous \"Yes\"", async () => {
    const { ask } = setup();
    const first = ask();
    await wrapper.get("#ports-keep-yes").trigger("click");
    await first;
    const second = ask();
    wrapper.get("dialog").element.dispatchEvent(new Event("close"));
    expect(await second).toBe("cancel");
  });

  it("removal from the screen while the question is open answers cancel, it doesn't hang", async () => {
    const { ask } = setup();
    const answer = ask();
    wrapper.unmount();
    expect(await answer).toBe("cancel");
    wrapper = mount(PortsKeepDialog, { attachTo: document.body });
  });

  it("no showModal — no right away", async () => {
    const { ask } = setup();
    wrapper.get("dialog").element.showModal = undefined;
    expect(await ask()).toBe("no");
  });
});

function setup() {
  wrapper = mount(PortsKeepDialog, { attachTo: document.body });
  return wrapper.vm;
}
