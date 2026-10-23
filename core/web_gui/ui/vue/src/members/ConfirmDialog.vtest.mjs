/* Contract of `members/ConfirmDialog.vue` (written before the component: red until it exists).
 *
 * Component: the one confirmation dialog of the "Members" tab (remove, confirm, block, leave), a native `<dialog>`.
 * No props. `defineExpose({ ask })`:
 *   ask(spec: { title: string; body: string[]; hint?: string; action: string; danger: boolean }): Promise<boolean>
 *   resolves true when the action is chosen, false for everything else (Cancel, Esc, a click on the backdrop,
 *   the component removed from the page while the question is open).
 * Markup: `dialog[role=alertdialog][aria-labelledby=confirm-title][aria-describedby=confirm-body]` (opened by `showModal()`);
 *   `#confirm-title` = spec.title; `#confirm-body` holds one `<p>` per body paragraph and, when given, the hint as `p.hint`;
 *   the action button `#member-confirm-yes` (class `btn-danger` when spec.danger) comes first in the DOM, then
 *   `#member-confirm-cancel` ("Cancel", class `btn-primary`, attribute `autofocus`, focused when the dialog opens).
 * Keyboard: Esc is the browser's own (`cancel`, then `close` with an empty returnValue) = false; Tab / Shift+Tab wrap around
 *   inside the dialog (the component handles the keydown itself); after closing the focus goes back to the element that was focused
 *   when `ask` was called. `.vtest.mjs`: happy-dom implements `showModal` (sets `open`) and `close` (fires `close`).
 * The optional reason field (`spec.field`, `reason()`): `ConfirmDialog.reason.vtest.mjs`. */

import { afterEach, describe, expect, it } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import ConfirmDialog from "./ConfirmDialog.vue";

const REMOVE = {
  title: "Remove peer-one from the network?",
  body: ["peer-one will lose access.", "On this computer, peer-one is blocked right away."],
  hint: "Use this if the computer is lost.",
  action: "Propose removal",
  danger: true,
};
const BLOCK = { title: "Block peer-one on this computer?", body: ["It won't be able to connect."], action: "Block peer-one", danger: false };

let wrapper;
let opener = null;
afterEach(() => {
  wrapper.unmount();
  if (opener) opener.remove();
  opener = null;
});

describe("ConfirmDialog", () => {
  describe("before the first question", () => {
    it("is not open", () => {
      const { page } = setup();
      expect(page.isOpen()).toBe(false);
    });
  });

  describe("a question is asked", () => {
    it("opens a modal alertdialog labelled by the title and described by the body", async () => {
      const { page } = setup();
      await page.ask(REMOVE);
      expect(page.isOpen()).toBe(true);
      expect(page.dialog().attributes("role")).toBe("alertdialog");
      expect(page.dialog().attributes("aria-labelledby")).toBe("confirm-title");
      expect(page.dialog().attributes("aria-describedby")).toBe("confirm-body");
    });

    it("shows the title, one paragraph per body line and the hint", async () => {
      const { page } = setup();
      await page.ask(REMOVE);
      expect(wrapper.get("#confirm-title").text()).toBe(REMOVE.title);
      expect(wrapper.findAll("#confirm-body p:not(.hint)").map((p) => p.text())).toEqual(REMOVE.body);
      expect(wrapper.get("#confirm-body .hint").text()).toBe(REMOVE.hint);
    });

    it("shows no hint when the spec has none", async () => {
      const { page } = setup();
      await page.ask(BLOCK);
      expect(wrapper.find("#confirm-body .hint").exists()).toBe(false);
    });

    it("names the action button after spec.action; a danger action is red", async () => {
      const { page } = setup();
      await page.ask(REMOVE);
      expect(page.yes().text()).toBe("Propose removal");
      expect(page.yes().classes()).toContain("btn-danger");
    });

    it("a non-danger action is not red", async () => {
      const { page } = setup();
      await page.ask(BLOCK);
      expect(page.yes().classes()).not.toContain("btn-danger");
    });

    it("Cancel is the primary button and comes after the action in the DOM", async () => {
      const { page } = setup();
      await page.ask(REMOVE);
      expect(page.cancel().text()).toBe("Cancel");
      expect(page.cancel().classes()).toContain("btn-primary");
      expect(page.cancel().attributes("autofocus")).toBeDefined();
      const buttons = wrapper.findAll("dialog button").map((b) => b.attributes("id"));
      expect(buttons).toEqual(["member-confirm-yes", "member-confirm-cancel"]);
    });

    it("moves the focus to Cancel", async () => {
      const { page } = setup();
      await page.ask(REMOVE);
      expect(document.activeElement).toBe(page.cancel().element);
    });

    it("shows the new text when the dialog is asked again with another spec", async () => {
      const { page } = setup();
      const { answer: first } = await page.ask(REMOVE);
      await page.clickCancel();
      await first;
      await page.ask(BLOCK);
      expect(wrapper.get("#confirm-title").text()).toBe(BLOCK.title);
      expect(page.yes().text()).toBe("Block peer-one");
      expect(wrapper.find("#confirm-body .hint").exists()).toBe(false);
    });
  });

  describe("the answer", () => {
    it("the action button answers true and closes the dialog", async () => {
      const { page } = setup();
      const { answer } = await page.ask(REMOVE);
      await page.clickYes();
      expect(await answer).toBe(true);
      expect(page.isOpen()).toBe(false);
    });

    it("Cancel answers false and closes the dialog", async () => {
      const { page } = setup();
      const { answer } = await page.ask(REMOVE);
      await page.clickCancel();
      expect(await answer).toBe(false);
      expect(page.isOpen()).toBe(false);
    });

    it("Esc answers false", async () => {
      const { page } = setup();
      const { answer } = await page.ask(REMOVE);
      page.pressEscape();
      expect(await answer).toBe(false);
    });

    it("Esc after an earlier \"yes\" answers false, not the previous true", async () => {
      const { page } = setup();
      const { answer: first } = await page.ask(REMOVE);
      await page.clickYes();
      await first;
      const { answer: second } = await page.ask(REMOVE);
      page.pressEscape();
      expect(await second).toBe(false);
    });

    it("a click on the backdrop (the dialog itself) answers false", async () => {
      const { page } = setup();
      const { answer } = await page.ask(REMOVE);
      await page.dialog().trigger("click");
      expect(await answer).toBe(false);
      expect(page.isOpen()).toBe(false);
    });

    it("a click on the text inside the dialog keeps it open", async () => {
      const { page } = setup();
      const { answer } = await page.ask(REMOVE);
      await wrapper.get("#confirm-body").trigger("click");
      expect(page.isOpen()).toBe(true);
      await page.clickCancel();
      await answer;
    });

    it("removing the component while the question is open answers false, it does not hang", async () => {
      const { page } = setup();
      const { answer } = await page.ask(REMOVE);
      wrapper.unmount();
      expect(await answer).toBe(false);
      wrapper = mount(ConfirmDialog, { attachTo: document.body });
    });
  });

  describe("the focus", () => {
    it("Tab on Cancel (the last button) wraps to the action", async () => {
      const { page } = setup();
      await page.ask(REMOVE);
      await page.cancel().trigger("keydown", { key: "Tab" });
      expect(document.activeElement).toBe(page.yes().element);
    });

    it("Shift+Tab on the action (the first button) wraps to Cancel", async () => {
      const { page } = setup();
      await page.ask(REMOVE);
      page.yes().element.focus();
      await page.yes().trigger("keydown", { key: "Tab", shiftKey: true });
      expect(document.activeElement).toBe(page.cancel().element);
    });

    it("returns to the element that was focused when the question was asked", async () => {
      const { page } = setup();
      opener = page.addOutsideButton();
      opener.focus();
      const { answer } = await page.ask(REMOVE);
      await page.clickCancel();
      await answer;
      expect(document.activeElement).toBe(opener);
    });
  });
});

function setup() {
  wrapper = mount(ConfirmDialog, { attachTo: document.body });
  const dialog = () => wrapper.get("dialog");
  const page = {
    dialog,
    yes: () => wrapper.get("#member-confirm-yes"),
    cancel: () => wrapper.get("#member-confirm-cancel"),
    isOpen: () => wrapper.find("dialog[open]").exists(),
    // Asks and waits until the dialog is rendered and open; `answer` is the promise of the choice.
    async ask(spec) {
      const answer = wrapper.vm.ask(spec);
      await flushPromises();
      return { answer };
    },
    clickYes: () => page.yes().trigger("click"),
    clickCancel: () => page.cancel().trigger("click"),
    // What a browser does on Esc: the `cancel` event, and unless it is prevented, `close()` without a return value.
    pressEscape() {
      const cancel = new Event("cancel", { cancelable: true });
      dialog().element.dispatchEvent(cancel);
      if (!cancel.defaultPrevented) dialog().element.close();
    },
    addOutsideButton() {
      const button = document.createElement("button");
      document.body.append(button);
      return button;
    },
  };
  return { page };
}
