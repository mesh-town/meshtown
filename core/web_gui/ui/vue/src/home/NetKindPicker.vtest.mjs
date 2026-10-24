/* The choice of the network kind on the create forms (`NetKindPicker.vue`): two radio cards, the "can't change" hint and,
 * while Managed is chosen, the warning about the first admin's data. `v-model` is the kind, `name` — what is typed in the name field. */

import { mount } from "@vue/test-utils";
import { afterEach, describe, expect, it } from "vitest";
import { settle } from "../testing/page.mjs";
import NetKindPicker from "./NetKindPicker.vue";

const TITLE = "Who can invite and remove members?";
const PEER_TITLE = "Peer network";
const PEER_TEXT = "Any member can invite people. Removing someone takes two members' votes.";
const MANAGED_TITLE = "Managed network";
const MANAGED_TEXT = "Only admins invite and remove members. You become the first admin.";
const HINT = "You can't change this after the network is created.";

let mounted;

afterEach(() => mounted.wrapper.unmount());

describe("the picker", () => {
  describe("on its own", () => {
    it("asks who can invite and remove members", async () => {
      const picker = await setup();
      expect(picker.text()).toContain(TITLE);
    });

    it("has a Peer card and a Managed card, each with its explanation", async () => {
      const picker = await setup();
      expect(picker.cardText("kind-peer")).toContain(PEER_TITLE);
      expect(picker.cardText("kind-peer")).toContain(PEER_TEXT);
      expect(picker.cardText("kind-managed")).toContain(MANAGED_TITLE);
      expect(picker.cardText("kind-managed")).toContain(MANAGED_TEXT);
      expect(picker.cardText("kind-peer")).not.toContain(MANAGED_TEXT);
    });

    it("says that the choice is final", async () => {
      const picker = await setup();
      expect(picker.text()).toContain(HINT);
    });

    it("the two cards are radios of one group", async () => {
      const picker = await setup();
      const [peer, managed] = [picker.radio("kind-peer"), picker.radio("kind-managed")];
      expect(peer.attributes("type")).toBe("radio");
      expect(managed.attributes("type")).toBe("radio");
      expect(peer.attributes("name")).toBeTruthy();
      expect(managed.attributes("name")).toBe(peer.attributes("name"));
    });

    it("each card is a label, so a click anywhere on it chooses it", async () => {
      const picker = await setup();
      expect(picker.radio("kind-peer").element.closest("label"), "label of Peer").not.toBeNull();
      expect(picker.radio("kind-managed").element.closest("label"), "label of Managed").not.toBeNull();
      expect(picker.radio("kind-peer").element.closest("label")).not.toBe(picker.radio("kind-managed").element.closest("label"));
    });
  });

  describe("Peer is the value", () => {
    it("the Peer card is chosen, the Managed card is not", async () => {
      const picker = await setup({ modelValue: "Peer" });
      expect(picker.radio("kind-peer").element.checked).toBe(true);
      expect(picker.radio("kind-managed").element.checked).toBe(false);
    });

    it("there is no warning", async () => {
      const picker = await setup({ modelValue: "Peer", name: "work" });
      expect(picker.exists("#kind-managed-warn")).toBe(false);
    });

    it("a click on the Managed card says Managed", async () => {
      const picker = await setup({ modelValue: "Peer" });
      await picker.when.cardClicked(MANAGED_TEXT);
      expect(picker.emitted()).toEqual(["Managed"]);
    });

    it("a click on the card's title works too", async () => {
      const picker = await setup({ modelValue: "Peer" });
      await picker.when.cardClicked(MANAGED_TITLE);
      expect(picker.emitted()).toEqual(["Managed"]);
    });

    it("a click on the Peer card, already chosen, changes nothing", async () => {
      const picker = await setup({ modelValue: "Peer" });
      await picker.when.cardClicked(PEER_TEXT);
      expect(picker.emitted().filter((v) => v !== "Peer")).toEqual([]);
    });
  });

  describe("Managed is the value", () => {
    it("the Managed card is chosen, the Peer card is not", async () => {
      const picker = await setup({ modelValue: "Managed" });
      expect(picker.radio("kind-managed").element.checked).toBe(true);
      expect(picker.radio("kind-peer").element.checked).toBe(false);
    });

    it("warns that losing this computer's data leaves the network without an admin, with the typed name", async () => {
      const picker = await setup({ modelValue: "Managed", name: "work" });
      expect(picker.warning()).toBe(
        'If this computer\'s Mesh Town data is lost and nobody else is an admin, "work" can\'t be managed any more. Make a second admin soon.',
      );
    });

    it("without a typed name the warning says \"this network\"", async () => {
      const picker = await setup({ modelValue: "Managed", name: "" });
      // Whether "this network" is quoted like a name is not fixed; only the words are checked.
      expect(picker.warning()).toContain("this network");
      expect(picker.warning()).toMatch(/can't be managed any more\. Make a second admin soon\.$/);
    });

    it("a blank name counts as no name", async () => {
      const picker = await setup({ modelValue: "Managed", name: "   " });
      expect(picker.warning()).toContain("this network");
    });

    it("the warning follows the name as it is typed", async () => {
      const picker = await setup({ modelValue: "Managed", name: "wo" });
      await picker.wrapper.setProps({ name: "work" });
      expect(picker.warning()).toContain('"work" can\'t be managed');
    });

    it("a click on the Peer card says Peer", async () => {
      const picker = await setup({ modelValue: "Managed" });
      await picker.when.cardClicked(PEER_TEXT);
      expect(picker.emitted()).toEqual(["Peer"]);
    });
  });

  describe("used as a v-model", () => {
    it("choosing Managed shows the warning, choosing Peer takes it away", async () => {
      const picker = await setup({ modelValue: "Peer", name: "work", controlled: true });
      expect(picker.exists("#kind-managed-warn")).toBe(false);
      await picker.when.cardClicked(MANAGED_TEXT);
      expect(picker.radio("kind-managed").element.checked).toBe(true);
      expect(picker.exists("#kind-managed-warn")).toBe(true);
      await picker.when.cardClicked(PEER_TEXT);
      expect(picker.radio("kind-peer").element.checked).toBe(true);
      expect(picker.exists("#kind-managed-warn")).toBe(false);
    });
  });
});

// Preconditions: the value, the typed name; `controlled` — the parent writes the emitted value back, as `v-model` does.
async function setup({ modelValue = "Peer", name = "", controlled = false } = {}) {
  const seen = [];
  const props = {
    modelValue,
    name,
    "onUpdate:modelValue": (value) => {
      seen.push(value);
      if (controlled) wrapper.setProps({ modelValue: value });
    },
  };
  const wrapper = mount(NetKindPicker, { props, attachTo: document.body });
  await settle();
  mounted = { wrapper };
  return pickerOf(wrapper, seen);
}

const compact = (text) => text.replace(/\s+/g, " ").trim();

function pickerOf(wrapper, seen) {
  const radio = (id) => wrapper.get("#" + id);
  const labelOf = (id) => radio(id).element.closest("label");
  return {
    wrapper,
    radio,
    text: () => compact(wrapper.text()),
    exists: (selector) => wrapper.find(selector).exists(),
    cardText: (id) => compact(labelOf(id).textContent),
    warning: () => compact(wrapper.get("#kind-managed-warn").text()),
    emitted: () => seen.slice(),
    when: {
      // Clicks the element inside a card that holds exactly this text (the label forwards the click to its radio).
      async cardClicked(text) {
        const inner = [...wrapper.element.querySelectorAll("label *")].find((e) => compact(e.textContent) === text);
        const target = inner || [...wrapper.element.querySelectorAll("label")].find((l) => compact(l.textContent).includes(text));
        expect(target, "a card part with the text " + text).toBeDefined();
        target.click();
        await settle();
      },
    },
  };
}
