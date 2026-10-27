/* The setup wizard and the main tab inside the Android app (user agent with `MeshTownApp/`): no browser setup, the path is
 * join, exit, connect; the last screen is a hint about the Connect button, the phone name lives in its own card under the setup. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { api } from "../core/api.ts";
import { apiTable, baseConfig, baseState, mountSection } from "../testing/section.mjs";
import { APP_UA, MARK_KEY, click, mountWizard, progress, step, unmountWizard } from "../testing/wizard.mjs";
import HomeSection from "./HomeSection.vue";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const WIZARD = '[role="region"][aria-labelledby="home-wizard-title"]';
let mounted;

beforeEach(() => {
  api.mockReset();
});
afterEach(() => unmountWizard(mounted));

describe("inside the Android app", () => {
  describe("the setup", () => {
    it("the title is \"Set up Mesh Town\"", () => {
      const { wrapper } = mountApp();
      expect(wrapper.get("#home-wizard-title").text()).toBe("Set up Mesh Town");
    });

    it("the first screen says the network is joined, with its name", () => {
      const { wrapper } = mountApp();
      expect(wrapper.get("#wizard-joined").text()).toBe("✓ You joined the network \"family\"");
    });

    it("app path is join, exit, connect with Done", async () => {
      const { wrapper } = mountApp();
      expect([step(wrapper), progress(wrapper)]).toEqual(["exit", "Step 2 of 3"]);
      expect(wrapper.get("#wizard-next").text()).toBe("Next");
      await click(wrapper, "#wizard-next");
      expect([step(wrapper), progress(wrapper)]).toEqual(["connect", "Step 3 of 3"]);
      expect(wrapper.get("#wizard-next").text()).toBe("Done");
    });

    it("the exit screen is \"Choose an exit\" with the exit list, and there is no browser setup", () => {
      const { wrapper } = mountApp();
      expect(wrapper.get("#wizard-q").text()).toBe("Choose an exit");
      expect(wrapper.get("#exit-explain").text()).toContain("An exit is a member whose internet");
      expect(wrapper.text()).not.toContain("Set up your browser");
      expect(wrapper.find("#pac-row").exists()).toBe(false);
      expect(wrapper.find("#scope-seg").exists()).toBe(false);
    });

    it("the exit screen has no \"This computer\" block and no name field: the name is in its own card", () => {
      const { wrapper } = mountApp();
      expect(wrapper.get(WIZARD).find("#field-node-name").exists()).toBe(false);
      expect(wrapper.get(WIZARD).text()).not.toContain("Name of this");
      expect(wrapper.get(WIZARD).text()).not.toContain("This computer");
    });

    it("the wizard has no exit server switch", () => {
      const { wrapper } = mountApp();
      expect(wrapper.find("#field-exit-enabled").exists()).toBe(false);
    });

    it("there is no way back from the first screen in a network", () => {
      const { wrapper } = mountApp();
      expect(wrapper.find("#wizard-back").exists()).toBe(false);
    });
  });

  describe("the Tap Connect screen", () => {
    it("says \"Tap Connect at the bottom of the screen\"", async () => {
      const { wrapper } = await mountConnect();
      expect(wrapper.get("#wizard-connect-step").text()).toBe("Tap Connect at the bottom of the screen");
    });

    it("has no check mark and no button of its own: it only tells where to tap", async () => {
      const { wrapper } = await mountConnect();
      const row = wrapper.get("#wizard-connect-step");
      expect(row.text()).not.toContain("✓");
      expect(row.element.tagName, "the line itself is not a button").not.toBe("BUTTON");
      expect(row.find("button").exists()).toBe(false);
    });

    it("a click on the line changes nothing: the screen stays, the mark is not written", async () => {
      const { wrapper } = await mountConnect();
      await click(wrapper, "#wizard-connect-step");
      expect(step(wrapper)).toBe("connect");
      expect(localStorage.getItem(MARK_KEY)).toBeNull();
    });

    it("Back returns to the exit screen", async () => {
      const { wrapper } = await mountConnect();
      await click(wrapper, "#wizard-back");
      expect(step(wrapper)).toBe("exit");
    });

    it("\"Done\" is the only way to finish in the app and writes the done mark", async () => {
      const { wrapper } = await mountConnect();
      const done = wrapper.get(WIZARD).findAll("button").filter((b) => b.text() === "Done");
      expect(done).toHaveLength(1);
      await done[0].trigger("click");
      expect(localStorage.getItem(MARK_KEY)).toBe("done");
    });
  });

  describe("the phone card under the setup", () => {
    it("is a separate card outside the setup card, titled \"This phone\"", () => {
      const { wrapper } = mountApp();
      const card = wrapper.get("#home-phone-card");
      expect(card.element.closest(WIZARD), "not inside the setup card").toBeNull();
      expect(card.get("h2").text()).toBe("This phone");
      expect(card.classes()).toContain("card");
    });

    it("is absent while the network is not joined", () => {
      const { wrapper } = (mounted = mountWizard({ inApp: true, hasNet: false }));
      expect(wrapper.find("#home-phone-card").exists()).toBe(false);
    });

    it("holds the field \"This phone's name\", the hint and a \"Save\" button after the field", () => {
      const { wrapper } = mountApp();
      const card = wrapper.get("#home-phone-card");
      expect(card.get("label[for=field-node-name]").text()).toBe("This phone's name");
      expect(card.get("#field-node-name").element.value).toBe("finch-unwary");
      expect(card.text()).toContain("Others see you by this name; leave it empty for a random one.");
      const save = saveButton(card);
      expect(save, "a Save button").toBeDefined();
      const follows = card.get("#field-node-name").element.compareDocumentPosition(save.element) & Node.DOCUMENT_POSITION_FOLLOWING;
      expect(follows, "the button comes after the field").toBeTruthy();
    });

    it("has no \"Allow others\" switch", () => {
      const { wrapper } = mountApp();
      expect(wrapper.get("#home-phone-card").find("#field-exit-enabled").exists()).toBe(false);
    });

    it("no green \"Saved\" before saving", () => {
      const { wrapper } = mountApp();
      expect(wrapper.get("#field-node-name-status").text()).toBe("");
      expect(wrapper.get("#field-node-name-status").attributes("hidden")).toBeDefined();
    });

    it("\"Save\" sends the new name and shows a green \"Saved\"", async () => {
      api.mockImplementation(apiTable({ "PUT /api/config": (body) => ({ config: body.config, rev: 2 }) }));
      const { wrapper } = mountApp();
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
      const { wrapper } = mountApp();
      await saveButton(wrapper.get("#home-phone-card")).trigger("click");
      await flushPromises();
      expect(nameCalls()).toHaveLength(0);
    });
  });

  describe("the main tab", () => {
    it("has no browser card", () => {
      setAppUserAgent();
      mounted = mountSection(HomeSection, { config: baseConfig(), state: baseState() });
      expect(mounted.wrapper.text()).not.toContain("Set up manually");
      expect(mounted.wrapper.text()).not.toContain("coming soon");
    });

    it("the name field says phone and the exit server switch is absent", () => {
      setAppUserAgent();
      mounted = mountSection(HomeSection, { config: baseConfig(), state: baseState() });
      expect(mounted.wrapper.find("label[for=field-node-name]").text()).toBe("This phone's name");
      expect(mounted.wrapper.find("#field-exit-enabled").exists()).toBe(false);
    });
  });
});

function setAppUserAgent() {
  Object.defineProperty(navigator, "userAgent", { value: APP_UA, configurable: true });
}

function mountApp() {
  return (mounted = mountWizard({ inApp: true, label: "family" }));
}

async function mountConnect() {
  const res = mountApp();
  await click(res.wrapper, "#wizard-next");
  return res;
}

// Mounting the exit list may auto-pick an exit with its own PUT; only the calls that carry the node name count.
function nameCalls() {
  return api.mock.calls.filter((c) => c[2] && c[2].config && c[2].config.node);
}

function saveButton(card) {
  return card.findAll("button").find((b) => b.text() === "Save");
}
