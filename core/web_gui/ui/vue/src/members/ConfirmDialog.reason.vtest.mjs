/* The optional reason field of `members/ConfirmDialog.vue` (the rest of the dialog: `ConfirmDialog.vtest.mjs`).
 *
 * Contract: when `spec.field` (`{ label, max }`) is given, the dialog draws `label[for=member-reason]` (the label text) and
 * `input#member-reason.input` (`type=text`, `maxlength=<max>`, `autocomplete=off`) before the buttons; without `spec.field` neither exists.
 * The input is empty on every opening. Enter in it confirms nothing (only the action button does). The focus still goes to Cancel;
 * Tab / Shift+Tab wrap around over the input and both buttons.
 * `defineExpose({ ask, reason })`: `reason()` is the text of the last opening cleaned by `cleanReason` (trimmed, control characters as
 * spaces, at most 80 code points); it is still there after the dialog closed (the caller reads it after the answer) and is "" when the
 * last opening had no field. */

import { afterEach, describe, expect, it } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import ConfirmDialog from "./ConfirmDialog.vue";

const FIELD = { label: "Why ban it? (optional)", max: 80 };
const BAN = {
  title: "Ban peer-one from the network?",
  body: ["peer-one will lose access.", "On this computer, peer-one is blocked right away."],
  hint: "Use this if the computer is lost.",
  action: "Propose ban",
  danger: true,
  field: FIELD,
};
const BLOCK = { title: "Block peer-one on this computer?", body: ["It won't be able to connect."], action: "Block peer-one", danger: false };

let wrapper;
let opener = null;
afterEach(() => {
  wrapper.unmount();
  if (opener) opener.remove();
  opener = null;
});

describe("ConfirmDialog with a reason field", () => {
  describe("a spec with the field", () => {
    it("draws a labelled text input before the buttons", async () => {
      const { page } = setup();
      await page.ask(BAN);
      expect(page.label().text()).toBe(FIELD.label);
      expect(page.label().attributes("for")).toBe("member-reason");
      expect(page.input().exists()).toBe(true);
      expect(page.isBefore(page.label(), page.input())).toBe(true);
      expect(page.isBefore(page.input(), page.yes())).toBe(true);
    });

    it("the input is a plain text field of at most 80 characters with no autocomplete", async () => {
      const { page } = setup();
      await page.ask(BAN);
      const input = page.input();
      expect(input.element.tagName).toBe("INPUT");
      expect(input.attributes("type")).toBe("text");
      expect(input.attributes("maxlength")).toBe("80");
      expect(input.attributes("autocomplete")).toBe("off");
      expect(input.classes()).toContain("input");
    });

    it("the focus is still on Cancel, not on the field", async () => {
      const { page } = setup();
      await page.ask(BAN);
      expect(document.activeElement).toBe(page.cancel().element);
    });

    it("the field is empty when the dialog opens", async () => {
      const { page } = setup();
      await page.ask(BAN);
      expect(page.input().element.value).toBe("");
    });

    it("the paragraphs and the hint are the same as without the field", async () => {
      const { page } = setup();
      await page.ask(BAN);
      expect(wrapper.findAll("#confirm-body p:not(.hint)").map((p) => p.text())).toEqual(BAN.body);
      expect(wrapper.get("#confirm-body .hint").text()).toBe(BAN.hint);
    });
  });

  describe("a spec without the field", () => {
    it("draws no input and no label", async () => {
      const { page } = setup();
      await page.ask(BLOCK);
      expect(page.input().exists()).toBe(false);
      expect(page.label().exists()).toBe(false);
    });

    it("the field of an earlier question is gone", async () => {
      const { page } = setup();
      const { answer: first } = await page.ask(BAN);
      await page.clickCancel();
      await first;
      await page.ask(BLOCK);
      expect(page.input().exists()).toBe(false);
      expect(page.label().exists()).toBe(false);
    });
  });

  describe("the text of the reason", () => {
    it("reason() gives what was typed", async () => {
      const { page } = setup();
      await page.ask(BAN);
      await page.type("lost laptop");
      expect(page.reason()).toBe("lost laptop");
    });

    it("is cleaned: trimmed, control characters become spaces", async () => {
      const { page } = setup();
      await page.ask(BAN);
      await page.type("  lost\tlaptop \u0007");
      expect(page.reason()).toBe("lost laptop");
    });

    it("is cut to 80 characters even if the field let more through (a paste)", async () => {
      const { page } = setup();
      await page.ask(BAN);
      await page.type("x".repeat(100));
      expect(page.reason()).toBe("x".repeat(80));
    });

    it("an untouched field gives an empty string", async () => {
      const { page } = setup();
      await page.ask(BAN);
      expect(page.reason()).toBe("");
    });

    it("a blank field gives an empty string", async () => {
      const { page } = setup();
      await page.ask(BAN);
      await page.type("   ");
      expect(page.reason()).toBe("");
    });

    it("is still there after the action button closed the dialog", async () => {
      const { page } = setup();
      const { answer } = await page.ask(BAN);
      await page.type("lost laptop");
      await page.clickYes();
      expect(await answer).toBe(true);
      expect(page.reason()).toBe("lost laptop");
    });

    it("is empty when the last question had no field", async () => {
      const { page } = setup();
      const { answer: first } = await page.ask(BAN);
      await page.type("lost laptop");
      await page.clickYes();
      await first;
      await page.ask(BLOCK);
      expect(page.reason()).toBe("");
    });
  });

  describe("the field is cleared on every opening", () => {
    it("after the action was chosen", async () => {
      const { page } = setup();
      const { answer: first } = await page.ask(BAN);
      await page.type("lost laptop");
      await page.clickYes();
      await first;
      await page.ask(BAN);
      expect(page.input().element.value).toBe("");
      expect(page.reason()).toBe("");
    });

    it("after Cancel", async () => {
      const { page } = setup();
      const { answer: first } = await page.ask(BAN);
      await page.type("lost laptop");
      await page.clickCancel();
      await first;
      await page.ask(BAN);
      expect(page.input().element.value).toBe("");
      expect(page.reason()).toBe("");
    });

    it("after Esc", async () => {
      const { page } = setup();
      const { answer: first } = await page.ask(BAN);
      await page.type("lost laptop");
      page.pressEscape();
      await first;
      await page.ask(BAN);
      expect(page.input().element.value).toBe("");
    });
  });

  describe("Enter in the field", () => {
    it("confirms nothing: the dialog stays open and the question stays unanswered", async () => {
      const { page } = setup();
      const { answer } = await page.ask(BAN);
      let answered = false;
      void answer.then(() => (answered = true));
      await page.type("lost laptop");
      await page.pressEnter();
      expect(answered, "Enter answered the question").toBe(false);
      expect(page.isOpen()).toBe(true);
      expect(wrapper.find("dialog form").exists(), "a form would submit on Enter").toBe(false);
      await page.clickCancel();
      await answer;
    });
  });

  describe("the focus with the field", () => {
    it("Tab on Cancel (the last control) wraps to the reason field", async () => {
      const { page } = setup();
      await page.ask(BAN);
      await page.cancel().trigger("keydown", { key: "Tab" });
      expect(document.activeElement).toBe(page.input().element);
    });

    it("Shift+Tab in the reason field (the first control) wraps to Cancel", async () => {
      const { page } = setup();
      await page.ask(BAN);
      page.input().element.focus();
      await page.input().trigger("keydown", { key: "Tab", shiftKey: true });
      expect(document.activeElement).toBe(page.cancel().element);
    });

    it("Tab in the reason field is left to the browser: it moves on to the action", async () => {
      const { page } = setup();
      await page.ask(BAN);
      page.input().element.focus();
      const event = await page.pressTab(page.input());
      expect(event.defaultPrevented, "Tab inside the dialog was cancelled").toBe(false);
    });

    it("returns to the element that was focused when the question was asked", async () => {
      const { page } = setup();
      opener = page.addOutsideButton();
      opener.focus();
      const { answer } = await page.ask(BAN);
      await page.type("lost laptop");
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
    yes: () => wrapper.get("#member-confirm-yes"),
    cancel: () => wrapper.get("#member-confirm-cancel"),
    input: () => wrapper.find("#member-reason"),
    label: () => wrapper.find("label[for='member-reason']"),
    isOpen: () => wrapper.find("dialog[open]").exists(),
    reason: () => wrapper.vm.reason(),
    type: (text) => page.input().setValue(text),
    // Asks and waits until the dialog is rendered and open; `answer` is the promise of the choice.
    async ask(spec) {
      const answer = wrapper.vm.ask(spec);
      await flushPromises();
      return { answer };
    },
    clickYes: () => page.yes().trigger("click"),
    clickCancel: () => page.cancel().trigger("click"),
    async pressEnter() {
      for (const type of ["keydown", "keypress", "keyup"]) await page.input().trigger(type, { key: "Enter", code: "Enter" });
    },
    // Tab pressed on `target`; the dialog's handler cancels it (`defaultPrevented`) only when it wraps the focus itself.
    async pressTab(target) {
      const event = new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true });
      target.element.dispatchEvent(event);
      await flushPromises();
      return event;
    },
    // What a browser does on Esc: the `cancel` event, and unless it is prevented, `close()` without a return value.
    pressEscape() {
      const cancel = new Event("cancel", { cancelable: true });
      dialog().element.dispatchEvent(cancel);
      if (!cancel.defaultPrevented) dialog().element.close();
    },
    isBefore: (first, second) => !!(first.element.compareDocumentPosition(second.element) & Node.DOCUMENT_POSITION_FOLLOWING),
    addOutsideButton() {
      const button = document.createElement("button");
      document.body.append(button);
      return button;
    },
  };
  return { page };
}
