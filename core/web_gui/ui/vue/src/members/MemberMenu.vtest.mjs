/* Contract of `members/MemberMenu.vue` (written before the component: red until it exists).
 *
 * Component: the "⋯" menu of one member row.
 * Props: `id: string` (the member's `id_full`), `name: string` (shown in the aria-label),
 *        `items: { key: string; label: string; danger?: boolean; disabled?: boolean; title?: string }[]`.
 * Emits: `pick(key)` when an enabled item is chosen (the menu closes).
 * Markup: the toggle `button.member-menu-btn[type=button][data-member-menu=<id>][aria-haspopup=menu][aria-expanded=true|false]
 *   [aria-label="More actions for <name>"]` with the text "⋯"; while open — `[role=menu]` with one
 *   `button[role=menuitem][data-member-item=<key>]` per item (a disabled item has `aria-disabled="true"` and its `title`, but is
 *   not a `disabled` button: it stays reachable by the arrows and a click on it does nothing and keeps the menu open);
 *   a danger item has the class `danger`. While closed there is no visible `[role=menu]`.
 * Keyboard: Enter / Space on the toggle open the menu (it is a native button, so a click is all a test can send); opening puts the
 *   focus on the first item; ArrowDown / ArrowUp move (wrapping), Home / End jump; Esc and Tab close the menu and put the focus
 *   back on the toggle; a click anywhere outside closes it; a click on the toggle toggles. The menu survives a re-render with new
 *   `items` (the page polls every 2 s) — the toggle element stays the same. */

import { afterEach, describe, expect, it } from "vitest";
import { DOMWrapper, flushPromises, mount } from "@vue/test-utils";
import MemberMenu from "./MemberMenu.vue";
import { shown } from "../testing/dom.mjs";

const PEER = "B".repeat(64);
const OTHER = "C".repeat(64);
const ITEMS = [
  { key: "block", label: "Block on this computer…" },
  { key: "exclude", label: "Ban from network…", danger: true },
];
const WAITING = [{ key: "waiting", label: "Waiting for another member to confirm", disabled: true, title: "Ban already proposed" }];

let wrappers = [];
afterEach(() => {
  wrappers.forEach((w) => w.unmount());
  wrappers = [];
});

describe("MemberMenu", () => {
  describe("closed", () => {
    it("shows a toggle that names the member and announces a menu", () => {
      const { menu } = setup();
      const toggle = menu.toggle();
      expect(toggle.text()).toBe("⋯");
      expect(toggle.attributes("type")).toBe("button");
      expect(toggle.attributes("data-member-menu")).toBe(PEER);
      expect(toggle.attributes("aria-haspopup")).toBe("menu");
      expect(toggle.attributes("aria-expanded")).toBe("false");
      expect(toggle.attributes("aria-label")).toBe("More actions for peer-one");
    });

    it("shows no menu", () => {
      const { menu } = setup();
      expect(menu.isOpen()).toBe(false);
    });
  });

  describe("opened by a click on the toggle", () => {
    it("lists the items in order as menu items", async () => {
      const { menu } = setup();
      await menu.open();
      expect(menu.toggle().attributes("aria-expanded")).toBe("true");
      expect(menu.isOpen()).toBe(true);
      expect(menu.labels()).toEqual(["Block on this computer…", "Ban from network…"]);
      expect(menu.items().map((i) => i.attributes("data-member-item"))).toEqual(["block", "exclude"]);
      expect(menu.items().every((i) => i.attributes("role") === "menuitem")).toBe(true);
    });

    it("moves the focus to the first item", async () => {
      const { menu } = setup();
      await menu.open();
      expect(menu.focused()).toBe("block");
    });

    it("marks a danger item", async () => {
      // NOTE: no class is fixed for the danger item; `mi danger` is used.
      const { menu } = setup();
      await menu.open();
      expect(menu.item("exclude").classes()).toContain("danger");
      expect(menu.item("block").classes()).not.toContain("danger");
    });

    it("a second click on the toggle closes it", async () => {
      const { menu } = setup();
      await menu.open();
      await menu.toggle().trigger("click");
      expect(menu.isOpen()).toBe(false);
      expect(menu.toggle().attributes("aria-expanded")).toBe("false");
    });

    it("the toggle is a native button, so Enter and Space open it too", () => {
      const { menu } = setup();
      expect(menu.toggle().element.tagName).toBe("BUTTON");
    });
  });

  describe("arrow keys", () => {
    it("ArrowDown moves to the next item and wraps to the first", async () => {
      const { menu } = setup();
      await menu.open();
      await menu.press("ArrowDown");
      expect(menu.focused()).toBe("exclude");
      await menu.press("ArrowDown");
      expect(menu.focused()).toBe("block");
    });

    it("ArrowUp moves to the previous item and wraps to the last", async () => {
      const { menu } = setup();
      await menu.open();
      await menu.press("ArrowUp");
      expect(menu.focused()).toBe("exclude");
      await menu.press("ArrowUp");
      expect(menu.focused()).toBe("block");
    });

    it("End jumps to the last item and Home to the first", async () => {
      const { menu } = setup();
      await menu.open();
      await menu.press("End");
      expect(menu.focused()).toBe("exclude");
      await menu.press("Home");
      expect(menu.focused()).toBe("block");
    });
  });

  describe("closing", () => {
    it("Esc closes the menu and puts the focus back on the toggle", async () => {
      const { menu } = setup();
      await menu.open();
      await menu.press("Escape");
      expect(menu.isOpen()).toBe(false);
      expect(menu.toggleHasFocus()).toBe(true);
    });

    it("Tab closes the menu and puts the focus on the toggle", async () => {
      const { menu } = setup();
      await menu.open();
      await menu.press("Tab");
      expect(menu.isOpen()).toBe(false);
      expect(menu.toggleHasFocus()).toBe(true);
    });

    it("a click outside closes the menu", async () => {
      const { menu } = setup();
      await menu.open();
      await menu.clickOutside();
      expect(menu.isOpen()).toBe(false);
      expect(menu.toggle().attributes("aria-expanded")).toBe("false");
    });

    it("opening another member's menu closes this one", async () => {
      const { menu, second } = setup({ withSecond: true });
      await menu.open();
      await second.open();
      expect(menu.isOpen()).toBe(false);
      expect(second.isOpen()).toBe(true);
    });
  });

  describe("choosing an item", () => {
    it("emits pick with the item's key and closes the menu", async () => {
      const { menu, wrapper } = setup();
      await menu.open();
      await menu.item("exclude").trigger("click");
      expect(wrapper.emitted("pick")).toEqual([["exclude"]]);
      expect(menu.isOpen()).toBe(false);
    });
  });

  describe("a disabled item", () => {
    it("has aria-disabled and a title, and is still a menu item", async () => {
      const { menu } = setup({ items: WAITING });
      await menu.open();
      const waiting = menu.item("waiting");
      expect(waiting.attributes("aria-disabled")).toBe("true");
      expect(waiting.attributes("title")).toBe("Ban already proposed");
      expect(waiting.attributes("role")).toBe("menuitem");
    });

    it("is not chosen by a click: no pick, the menu stays open", async () => {
      const { menu, wrapper } = setup({ items: WAITING });
      await menu.open();
      await menu.item("waiting").trigger("click");
      expect(wrapper.emitted("pick")).toBeUndefined();
      expect(menu.isOpen()).toBe(true);
    });
  });

  describe("the items change while the menu is open (a poll re-renders the row)", () => {
    it("the menu stays open and the toggle is the same element", async () => {
      const { menu, wrapper } = setup();
      await menu.open();
      const toggle = menu.toggle().element;
      await wrapper.setProps({ items: ITEMS.map((i) => ({ ...i })) });
      expect(menu.isOpen()).toBe(true);
      expect(menu.toggle().element).toBe(toggle);
      expect(toggle.isConnected).toBe(true);
    });

    it("shows the new items", async () => {
      const { menu, wrapper } = setup();
      await menu.open();
      await wrapper.setProps({ items: WAITING });
      expect(menu.labels()).toEqual(["Waiting for another member to confirm"]);
    });
  });
});

function mountMenu(id, items) {
  const wrapper = mount(MemberMenu, { props: { id, name: "peer-one", items }, attachTo: document.body });
  wrappers.push(wrapper);
  return wrapper;
}

function driver(wrapper, id) {
  const menu = {
    toggle: () => wrapper.get('button[data-member-menu="' + id + '"]'),
    isOpen() {
      const el = wrapper.find('[role="menu"]');
      return el.exists() && shown(el.element);
    },
    items: () => wrapper.findAll('[role="menuitem"]'),
    item: (key) => wrapper.get('[data-member-item="' + key + '"]'),
    labels: () => menu.items().map((i) => i.text()),
    focused: () => document.activeElement && document.activeElement.getAttribute("data-member-item"),
    toggleHasFocus: () => document.activeElement === menu.toggle().element,
    open: () => menu.toggle().trigger("click"),
    // A key goes to the element that has the focus, as in a browser.
    press: (key) => new DOMWrapper(document.activeElement).trigger("keydown", { key }),
    async clickOutside() {
      document.body.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await flushPromises();
    },
  };
  return menu;
}

function setup({ items = ITEMS, withSecond = false } = {}) {
  const wrapper = mountMenu(PEER, items);
  const menu = driver(wrapper, PEER);
  const second = withSecond ? driver(mountMenu(OTHER, ITEMS), OTHER) : null;
  return { menu, second, wrapper };
}
