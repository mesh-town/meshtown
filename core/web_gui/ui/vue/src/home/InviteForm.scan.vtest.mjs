/* The "Scan QR" button of the invite form: a link to the app scanner, present only when the page is open inside the Android
 * app; the same form serves Home, "Add another network" and the tab of a network this computer left. */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";
import { defineComponent, h } from "vue";
import { api } from "../core/api.ts";
import { createStore, STORE_KEY } from "../core/store.ts";
import InviteForm from "./InviteForm.vue";
import { useInviteJoin } from "./use-invite-join.ts";

vi.mock("../core/api.ts", async (orig) => ({ ...(await orig()), api: vi.fn() }));

const APP_UA = "Mozilla/5.0 (Linux; Android 14) Chrome/120 Mobile MeshTownApp/Android";
const BROWSER_UA = "Mozilla/5.0 (Linux; Android 14) Chrome/120 Mobile Safari/537.36";
const SCAN = "a.invite-scan";
const PLACES = [
  ["Home", { kind: "home" }],
  ["Add another network", { kind: "add" }],
  ["a network this computer left", { kind: "rejoin", slot: "neta22" }],
];
const IDS = { input: "x-input", button: "x-btn", error: "x-error", preview: "x-preview", note: "x-note", dup: "x-dup", open: "x-open", status: "x-status", title: "x-title" };
let wrapper;

beforeEach(() => {
  api.mockReset();
  api.mockResolvedValue(null);
  history.replaceState(null, "", "/");
});
afterEach(() => {
  wrapper.unmount();
  Reflect.deleteProperty(navigator, "userAgent");
});

describe("invite form: Scan QR", () => {
  describe("inside the Android app", () => {
    it("shows \"Scan QR\" linking to the app scanner", () => {
      const form = mountForm(APP_UA);
      const link = form.scan();
      expect(link.exists(), "the link is there").toBe(true);
      expect(link.element.tagName).toBe("A");
      expect(link.text()).toBe("Scan QR");
      expect(link.attributes("href")).toBe("meshtown-app://scan-invite");
      expect(link.attributes("role")).toBe("button");
    });

    it("puts the link in the button row, right after the join button", () => {
      const form = mountForm(APP_UA);
      expect(form.button().element.nextElementSibling).toBe(form.scan().element);
    });

    it("leaves the join button as it was", () => {
      const form = mountForm(APP_UA);
      expect(form.button().text()).toBe("Join");
      expect(form.button().element.tagName).toBe("BUTTON");
    });

    it("clicking the link asks the node for nothing", async () => {
      const form = mountForm(APP_UA);
      await form.scan().trigger("click");
      expect(api).not.toHaveBeenCalled();
    });

    for (const [place, ctx] of PLACES) {
      it(`${place}: the form has the link`, () => {
        const form = mountForm(APP_UA, ctx);
        expect(form.scan().attributes("href")).toBe("meshtown-app://scan-invite");
      });
    }
  });

  describe("in a browser", () => {
    it("has no \"Scan QR\" in a browser", () => {
      const form = mountForm(BROWSER_UA);
      expect(form.scan().exists(), "the link").toBe(false);
      expect(wrapper.text()).not.toContain("Scan QR");
      expect(wrapper.html()).not.toContain("meshtown-app://");
    });

    it("keeps the join button on its own in the row", () => {
      const form = mountForm(BROWSER_UA);
      expect(form.button().element.nextElementSibling).toBeNull();
    });

    for (const [place, ctx] of PLACES) {
      it(`${place}: the form has no link`, () => {
        const form = mountForm(BROWSER_UA, ctx);
        expect(form.scan().exists()).toBe(false);
      });
    }
  });
});

function mountForm(userAgent, ctx = { kind: "add" }) {
  Object.defineProperty(navigator, "userAgent", { value: userAgent, configurable: true });
  const store = createStore();
  const Host = defineComponent({
    setup() {
      const join = useInviteJoin(store, ctx);
      return () => h(InviteForm, { join, ids: IDS, title: "A title" });
    },
  });
  wrapper = mount(Host, { attachTo: document.body, global: { provide: { [STORE_KEY]: store } } });
  return { scan: () => wrapper.find(SCAN), button: () => wrapper.get("#x-btn") };
}
