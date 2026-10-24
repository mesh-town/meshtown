/* Contract of `members/ExcludedCard.vue` (written before the component: red until it exists).
 *
 * Component: the screen of a computer that is no longer in the network; it takes the place of the Members content and of
 * the Home cards.
 * Props: `variant: "banned" | "left" | "forgotten"` (the members banned this computer / it left on its own / the members
 *   removed it from the member list after it had been offline), `net: string` (the network name, shown in quotes).
 * Emits: `join` (the "Join with another invite" button, `#excluded-join`); the parent switches to Home and focuses the invite field.
 * Markup: `section#excluded-card[role=status]` with a title and a text.
 * The "forgotten" variant has one button, "Join with an invite" (`#excluded-join`), also inside the app: there is no "Start over".
 * NOTE: there is no body text for the "left" variant (only the title `You left the network "{net}"`), so it is not asserted. */

import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import ExcludedCard from "./ExcludedCard.vue";

let wrapper;
afterEach(() => wrapper.unmount());

describe("ExcludedCard", () => {
  describe("variant banned", () => {
    it("is a status region that says the computer was banned from the network", () => {
      setup({ variant: "banned" });
      const card = wrapper.get("#excluded-card");
      expect(card.attributes("role")).toBe("status");
      expect(card.text()).toContain('This computer was banned from the network "demo-network"');
    });

    it("explains the consequences and whom to ask", () => {
      setup({ variant: "banned" });
      const text = wrapper.get("#excluded-card").text();
      expect(text).toContain("Members of the network banned it. Sites no longer open through Mesh Town, and members can't reach this computer.");
      expect(text).toContain("If you think this is a mistake, contact a member of the network.");
    });
  });

  describe("variant banned in a Managed network", () => {
    it("says an admin removed the computer", () => {
      setup({ variant: "banned", managed: true });
      const text = wrapper.get("#excluded-card").text();
      expect(text).toContain("An admin removed it. Sites no longer open through Mesh Town, and members can't reach this computer.");
      expect(text).not.toContain("Members of the network banned it");
    });
  });

  describe("variant left", () => {
    it("says the computer left the network", () => {
      setup({ variant: "left" });
      expect(wrapper.get("#excluded-card").text()).toContain('You left the network "demo-network"');
    });

    it("does not claim that the members banned it", () => {
      setup({ variant: "left" });
      const text = wrapper.get("#excluded-card").text();
      expect(text).not.toContain("was banned from the network");
      expect(text).not.toContain("Members of the network banned it");
    });
  });

  describe("variant forgotten", () => {
    it("says the computer was removed from the member list", () => {
      setup({ variant: "forgotten" });
      const card = wrapper.get("#excluded-card");
      expect(card.attributes("role")).toBe("status");
      expect(card.text()).toContain('This computer was removed from the member list "demo-network"');
    });

    it("explains that a new invite is the way back", () => {
      setup({ variant: "forgotten" });
      expect(wrapper.get("#excluded-card").text()).toContain("Ask a member for a new invite");
    });

    it("does not claim a ban or a departure", () => {
      setup({ variant: "forgotten" });
      const text = wrapper.get("#excluded-card").text();
      expect(text).not.toContain("banned");
      expect(text).not.toContain("You left the network");
    });

    it("inside the app the join button stays and there is no Start over link", () => {
      Object.defineProperty(navigator, "userAgent", { value: "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android", configurable: true });
      try {
        setup({ variant: "forgotten" });
        expect(wrapper.find("#excluded-start-over").exists()).toBe(false);
        expect(wrapper.get("#excluded-join").text()).toBe("Join with an invite");
      } finally {
        Reflect.deleteProperty(navigator, "userAgent");
      }
    });

    it("forgotten variant has join only", async () => {
      setup({ variant: "forgotten" });
      expect(wrapper.findAll("button, a")).toHaveLength(1);
      const button = wrapper.get("#excluded-join");
      expect(button.text()).toBe("Join with an invite");
      expect(button.classes()).toContain("btn-primary");
      expect(wrapper.find("#excluded-start-over").exists()).toBe(false);
      await button.trigger("click");
      expect(wrapper.emitted("join")).toHaveLength(1);
    });
  });

  describe("the button", () => {
    it.each(["banned", "left"])("%s: \"Join with another invite\" is the primary button and emits join", async (variant) => {
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
      setup({ variant: "banned", net: "office" });
      expect(wrapper.get("#excluded-card").text()).toContain('"office"');
    });
  });
});

function setup({ variant, net = "demo-network", managed }) {
  wrapper = mount(ExcludedCard, { props: { variant, net, managed }, attachTo: document.body });
}
