/* Contract of `members/ExcludedCard.vue` (written before the component: red until it exists).
 *
 * Component: the screen of a computer that is no longer in the network; it takes the place of the Members content and of
 * the Home cards.
 * Props: `variant: "removed" | "left"` (the members removed this computer / it left on its own), `net: string` (the network
 *   name, shown in quotes).
 * Emits: `join` (the "Join with another invite" button, `#excluded-join`); the parent switches to Home and focuses the invite field.
 * Markup: `section#excluded-card[role=status]` with a title and a text.
 * NOTE: there is no body text for the "left" variant (only the title `You left the network "{net}"`), so it is not asserted. */

import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import ExcludedCard from "./ExcludedCard.vue";

let wrapper;
afterEach(() => wrapper.unmount());

describe("ExcludedCard", () => {
  describe("variant removed", () => {
    it("is a status region that says the computer was removed from the network", () => {
      setup({ variant: "removed" });
      const card = wrapper.get("#excluded-card");
      expect(card.attributes("role")).toBe("status");
      expect(card.text()).toContain('This computer was removed from the network "demo-network"');
    });

    it("explains the consequences and whom to ask", () => {
      setup({ variant: "removed" });
      const text = wrapper.get("#excluded-card").text();
      expect(text).toContain("Members of the network removed it. Sites no longer open through Mesh Town, and members can't reach this computer.");
      expect(text).toContain("If you think this is a mistake, contact a member of the network.");
    });
  });

  describe("variant left", () => {
    it("says the computer left the network", () => {
      setup({ variant: "left" });
      expect(wrapper.get("#excluded-card").text()).toContain('You left the network "demo-network"');
    });

    it("does not claim that the members removed it", () => {
      setup({ variant: "left" });
      const text = wrapper.get("#excluded-card").text();
      expect(text).not.toContain("was removed from the network");
      expect(text).not.toContain("Members of the network removed it");
    });
  });

  describe("the button", () => {
    it.each(["removed", "left"])("%s: \"Join with another invite\" is the primary button and emits join", async (variant) => {
      setup({ variant });
      const button = wrapper.get("#excluded-join");
      expect(button.text()).toBe("Join with another invite");
      expect(button.classes()).toContain("btn-primary");
      await button.trigger("click");
      expect(wrapper.emitted("join")).toHaveLength(1);
    });
  });

  describe("the network name", () => {
    it("is the one given in the props", () => {
      setup({ variant: "removed", net: "office" });
      expect(wrapper.get("#excluded-card").text()).toContain('"office"');
    });
  });
});

function setup({ variant, net = "demo-network" }) {
  wrapper = mount(ExcludedCard, { props: { variant, net }, attachTo: document.body });
}
