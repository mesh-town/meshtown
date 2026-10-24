/* Contract of `members/VoteBanner.vue` (written before the component: red until it exists).
 *
 * Component: the banner at the top of the "Members" tab, shown to a member who can confirm somebody's ban.
 * Props: `by: string` (display name of the member who proposed), `target: string` (display name of the member to ban),
 *   `reason?: string | null` (what the proposer wrote; with text, `p.vote-reason` says `Reason from {by}: "{reason}"` — as text, never as markup;
 *   absent, null or empty — no such paragraph).
 * Emits: `confirm` (the "Confirm ban…" button, `#vote-confirm`), `later` (the "Not now" button, `#vote-later`).
 *   The component only reports the clicks: opening the dialog and hiding the banner until the next page load are the parent's job.
 * Markup: `section#vote-banner[role=region][aria-labelledby=vote-title][tabindex=-1]` (focusable by script: the tray link
 *   focuses it), title `#vote-title`, then the explanation paragraph and the two buttons. */

import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import VoteBanner from "./VoteBanner.vue";

let wrapper;
afterEach(() => wrapper.unmount());

describe("VoteBanner", () => {
  describe("for a proposal by peer-two to ban peer-one", () => {
    it("is a labelled region that can take the focus", () => {
      setup();
      const banner = wrapper.get("#vote-banner");
      expect(banner.element.tagName).toBe("SECTION");
      expect(banner.attributes("role")).toBe("region");
      expect(banner.attributes("aria-labelledby")).toBe("vote-title");
      expect(banner.attributes("tabindex")).toBe("-1");
    });

    it("says who proposes banning whom", () => {
      setup();
      expect(wrapper.get("#vote-title").text()).toBe("peer-two proposes banning peer-one from the network");
    });

    it("explains what confirming and not answering do", () => {
      setup();
      const text = wrapper.get("#vote-banner").text();
      expect(text).toContain("If you confirm, peer-one is banned for everyone and its open connections are cut.");
      expect(text).toContain("If you don't answer, nothing changes for you: peer-one stays in the network until another member confirms.");
    });

    it("offers \"Confirm ban…\" and \"Not now\"", () => {
      setup();
      expect(wrapper.get("#vote-confirm").text()).toBe("Confirm ban…");
      expect(wrapper.get("#vote-later").text()).toBe("Not now");
    });
  });

  describe("the proposer gave a reason", () => {
    it("shows it in its own paragraph, with the name of the proposer", () => {
      setup({ by: "peer-two", target: "peer-one", reason: "lost laptop" });
      expect(wrapper.get("#vote-banner p.vote-reason").text()).toBe('Reason from peer-two: "lost laptop"');
    });

    it("keeps the title, the explanation and both buttons", () => {
      setup({ by: "peer-two", target: "peer-one", reason: "lost laptop" });
      expect(wrapper.get("#vote-title").text()).toBe("peer-two proposes banning peer-one from the network");
      expect(wrapper.get("#vote-banner").text()).toContain("If you confirm, peer-one is banned for everyone");
      expect(wrapper.get("#vote-confirm").text()).toBe("Confirm ban…");
      expect(wrapper.get("#vote-later").text()).toBe("Not now");
    });

    it("shows markup in the reason as plain text, not as elements", () => {
      setup({ by: "peer-two", target: "peer-one", reason: '<b>bold</b> <img src=x onerror="alert(1)"> &amp;' });
      const paragraph = wrapper.get("p.vote-reason");
      expect(paragraph.text()).toBe('Reason from peer-two: "<b>bold</b> <img src=x onerror="alert(1)"> &amp;"');
      expect(paragraph.find("b").exists()).toBe(false);
      expect(paragraph.find("img").exists()).toBe(false);
      expect(wrapper.get("#vote-banner").find("img").exists()).toBe(false);
    });

    it("the name of the proposer is not read as markup either", () => {
      setup({ by: "<i>peer</i>", target: "peer-one", reason: "lost laptop" });
      expect(wrapper.get("p.vote-reason").text()).toBe('Reason from <i>peer</i>: "lost laptop"');
      expect(wrapper.get("p.vote-reason").find("i").exists()).toBe(false);
    });
  });

  describe("the proposer gave no reason", () => {
    it.each([["is not given", undefined], ["is null", null], ["is empty", ""]])("a reason that %s: no reason paragraph", (_name, reason) => {
      setup({ by: "peer-two", target: "peer-one", reason });
      expect(wrapper.find(".vote-reason").exists()).toBe(false);
      expect(wrapper.get("#vote-banner").text()).not.toContain("Reason from");
    });
  });

  describe("the buttons", () => {
    it("\"Confirm ban…\" emits confirm and nothing else", async () => {
      setup();
      await wrapper.get("#vote-confirm").trigger("click");
      expect(wrapper.emitted("confirm")).toHaveLength(1);
      expect(wrapper.emitted("later")).toBeUndefined();
    });

    it("\"Not now\" emits later and nothing else", async () => {
      setup();
      await wrapper.get("#vote-later").trigger("click");
      expect(wrapper.emitted("later")).toHaveLength(1);
      expect(wrapper.emitted("confirm")).toBeUndefined();
    });
  });
});

function setup(props = { by: "peer-two", target: "peer-one" }) {
  wrapper = mount(VoteBanner, { props, attachTo: document.body });
}
