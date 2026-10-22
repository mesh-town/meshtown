/* The "Three steps" setup and the main tab inside the Android app (user agent with `MeshTownApp/`): no browser setup,
 * the third step is a hint about the Connect button, the phone name lives in its own card under the setup. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { apiTable, baseConfig, baseState, mountSection } from "../testing/section.mjs";
import HomeSection from "./HomeSection.vue";
import SetupWizard from "./SetupWizard.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const MARK = "meshtown.homeSetup";
const WIZARD = '[role="region"][aria-labelledby="home-wizard-title"]';
const FAMILY = { id_hex: "net-aaaa", label: "family" };
let mounted;
const setUa = (ua) => Object.defineProperty(navigator, "userAgent", { value: ua, configurable: true });

beforeEach(() => {
  localStorage.clear();
  setUa("Mozilla/5.0 (Linux; Android 14) Chrome/120 Mobile MeshTownApp/Android");
});
afterEach(() => {
  mounted.wrapper.unmount();
  Reflect.deleteProperty(navigator, "userAgent");
});

describe("inside the Android app", () => {
  describe("the setup card", () => {
    it("the title is \"Three steps to open a blocked site\"", () => {
      const { wrapper } = mountWizard();
      expect(wrapper.get("#home-wizard-title").text()).toBe("Three steps to open a blocked site");
    });

    it("step 1 says the network is joined, with its name", () => {
      const { wrapper } = mountWizard();
      expect(wrapper.get(WIZARD).text()).toContain("✓ You joined the network \"family\"");
    });

    it("step 2 is \"Choose an exit\" with the exit list, and there is no browser step", () => {
      const { wrapper } = mountWizard();
      expect(wrapper.find("#wizard-step-2").exists()).toBe(false);
      expect(wrapper.get("#wizard-step-3").text()).toBe("2Choose an exit");
      expect(wrapper.get("#exit-explain").text()).toContain("An exit is a member whose internet");
      expect(wrapper.text()).not.toContain("Set up your browser");
      expect(wrapper.find("#pac-row").exists()).toBe(false);
    });

    it("step 2 has no name field: the name is in its own card", () => {
      const { wrapper } = mountWizard();
      expect(wrapper.get(WIZARD).find("#field-node-name").exists()).toBe(false);
      expect(wrapper.get(WIZARD).text()).not.toContain("Name of this");
    });

    it("the wizard has no exit server switch", async () => {
      const { wrapper } = mountWizard();
      await wrapper.get("#wizard-step-3").trigger("click");
      expect(wrapper.find("#field-exit-enabled").exists()).toBe(false);
    });
  });

  describe("step 3: Tap Connect", () => {
    it("is a row numbered 3 with the words \"Tap Connect at the bottom of the screen\"", () => {
      const { wrapper } = mountWizard();
      const row = wrapper.get("#wizard-connect-step");
      expect(row.text()).toBe("3Tap Connect at the bottom of the screen");
      expect(row.get(".step-num").text()).toBe("3");
    });

    it("has no check mark and no button: it only tells where to tap", () => {
      const { wrapper } = mountWizard();
      const row = wrapper.get("#wizard-connect-step");
      expect(row.text()).not.toContain("✓");
      expect(row.element.tagName, "the row itself is not a button").not.toBe("BUTTON");
      expect(row.find("button").exists()).toBe(false);
    });

    it("a click on the row changes nothing: the exit step stays open, the mark is not written", async () => {
      const { wrapper } = mountWizard();
      await wrapper.get("#wizard-connect-step").trigger("click");
      expect(wrapper.find("#home-exit").exists()).toBe(true);
      expect(localStorage.getItem(MARK)).toBeNull();
    });

    it("\"Done\" stays under step 3: without it the setup never finishes in the app", async () => {
      // The setup ends only by the mark written from "Done"; nothing else writes it, so the button is kept.
      const { wrapper } = mountWizard();
      const done = wrapper.findAll("button").filter((b) => b.text() === "Done");
      expect(done).toHaveLength(1);
      await done[0].trigger("click");
      expect(localStorage.getItem(MARK)).toBe("done");
    });
  });

  describe("the phone card under the setup", () => {
    it("is a separate card outside the setup card, titled \"This phone\"", () => {
      const { wrapper } = mountWizard();
      const card = wrapper.get("#home-phone-card");
      expect(card.element.closest(WIZARD), "not inside the setup card").toBeNull();
      expect(card.get("h2").text()).toBe("This phone");
      expect(card.classes()).toContain("card");
    });

    it("holds the field \"Name of this phone\", the hint and a \"Save\" button after the field", () => {
      const { wrapper } = mountWizard();
      const card = wrapper.get("#home-phone-card");
      expect(card.get("label[for=field-node-name]").text()).toBe("Name of this phone");
      expect(card.get("#field-node-name").element.value).toBe("finch-unwary");
      expect(card.text()).toContain("Others see you by this name; leave it empty for a random one.");
      const save = saveButton(card);
      expect(save, "a Save button").toBeDefined();
      const follows = card.get("#field-node-name").element.compareDocumentPosition(save.element) & Node.DOCUMENT_POSITION_FOLLOWING;
      expect(follows, "the button comes after the field").toBeTruthy();
    });

    it("has no \"Allow others\" switch", () => {
      const { wrapper } = mountWizard();
      expect(wrapper.get("#home-phone-card").find("#field-exit-enabled").exists()).toBe(false);
    });

    it("no green \"Saved\" before saving", () => {
      const { wrapper } = mountWizard();
      expect(wrapper.get("#field-node-name-status").text()).toBe("");
      expect(wrapper.get("#field-node-name-status").attributes("hidden")).toBeDefined();
    });

    it("\"Save\" sends the new name and shows a green \"Saved\"", async () => {
      api.mockImplementation(apiTable({ "PUT /api/config": (body) => ({ config: body.config, rev: 2 }) }));
      const { wrapper } = mountWizard();
      await wrapper.get("#field-node-name").setValue("my-phone");
      await saveButton(wrapper.get("#home-phone-card")).trigger("click");
      await flushPromises();
      expect(nameCalls()).toHaveLength(1);
      expect(nameCalls()[0][2].config.node.name).toBe("my-phone");
      const status = wrapper.get("#field-node-name-status");
      expect(status.text()).toBe("Saved");
      expect(status.classes(), "green").toContain("saved");
    });

    it("\"Save\" with an unchanged name sends nothing", async () => {
      const { wrapper } = mountWizard();
      await saveButton(wrapper.get("#home-phone-card")).trigger("click");
      await flushPromises();
      expect(nameCalls()).toHaveLength(0);
    });
  });

  describe("the main tab", () => {
    it("has no browser card", () => {
      mounted = mountSection(HomeSection, { config: baseConfig(), state: baseState() });
      expect(mounted.wrapper.text()).not.toContain("Set up manually");
      expect(mounted.wrapper.text()).not.toContain("coming soon");
    });

    it("the name field says phone and the exit server switch is absent", () => {
      mounted = mountSection(HomeSection, { config: baseConfig(), state: baseState() });
      expect(mounted.wrapper.find("label[for=field-node-name]").text()).toBe("Name of this phone");
      expect(mounted.wrapper.find("#field-exit-enabled").exists()).toBe(false);
    });
  });
});

function mountWizard() {
  return (mounted = mountSection(SetupWizard, { state: baseState({ network: FAMILY }) }));
}

// Mounting the exit list may auto-pick an exit with its own PUT; only the calls that carry the node name count.
function nameCalls() {
  return api.mock.calls.filter((c) => c[2] && c[2].config && c[2].config.node);
}

function saveButton(card) {
  return card.findAll("button").find((b) => b.text() === "Save");
}
