/* `TermText`: the words PAC and SOCKS5 become focusable terms with a tooltip; other text stays plain. */

import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import TermText from "./TermText.vue";
import { TERM_TIPS } from "./term-hints.ts";

const wrappers = [];
afterEach(() => wrappers.splice(0).forEach((w) => w.unmount()));

describe("TermText", () => {
  describe("a text with a term", () => {
    it("wraps the term in a focusable span described by its tooltip", () => {
      const wrapper = mountText("Copy this address (PAC): now");
      const term = wrapper.get("span.term");
      expect(term.attributes("tabindex")).toBe("0");
      const tip = term.get('[role="tooltip"]');
      expect(tip.attributes("id"), "the tooltip has an id").toBeTruthy();
      expect(term.attributes("aria-describedby")).toBe(tip.attributes("id"));
      expect(tip.text()).toBe(TERM_TIPS.PAC);
    });

    it("keeps the visible sentence as it was", () => {
      const wrapper = mountText("Copy this address (PAC): now");
      expect(wrapper.text()).toContain("Copy this address (PAC");
      expect(wrapper.text()).toContain("): now");
    });

    it("two instances get different tooltip ids", () => {
      const pair = mount({ components: { TermText }, template: '<div><TermText text="PAC" /><TermText text="PAC" /></div>' });
      wrappers.push(pair);
      const ids = pair.findAll('[role="tooltip"]').map((tip) => tip.attributes("id"));
      expect(ids).toHaveLength(2);
      expect(ids[0]).not.toBe(ids[1]);
    });
  });

  describe("a text without a term", () => {
    it("has no .term and no tooltip", () => {
      const wrapper = mountText("Choose an exit");
      expect(wrapper.find(".term").exists()).toBe(false);
      expect(wrapper.find('[role="tooltip"]').exists()).toBe(false);
      expect(wrapper.text()).toBe("Choose an exit");
    });

    it("PACs is not wrapped", () => {
      expect(mountText("PACs").find(".term").exists()).toBe(false);
    });
  });

  describe("the keyboard", () => {
    it("Escape on a focused term takes the focus away", async () => {
      const wrapper = mountText("SOCKS5");
      const term = wrapper.get("span.term");
      term.element.focus();
      expect(document.activeElement, "the term is focused").toBe(term.element);
      await term.trigger("keydown", { key: "Escape" });
      expect(document.activeElement).not.toBe(term.element);
    });
  });
});

function mountText(text) {
  const wrapper = mount(TermText, { props: { text }, attachTo: document.body });
  wrappers.push(wrapper);
  return wrapper;
}
