/* `ConfirmDialog.vue` for a managed network: the typed-name gate of "Delete network" (`field.match`) and the second
 * action button (`spec.alt`, `alt()`). The rest of the dialog: `ConfirmDialog.vtest.mjs`, `ConfirmDialog.reason.vtest.mjs`. */

import { afterEach, describe, expect, it } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import ConfirmDialog from "./ConfirmDialog.vue";

const DELETE = {
  title: 'Delete "work" for everyone?',
  body: ["All 4 members leave."],
  field: { label: "Type the network name to confirm", max: 200, match: "work" },
  action: "Delete network",
  danger: true,
};
const LAST = { title: "You're the only admin", body: ["Choose first."], action: "Choose an admin", alt: { label: "Delete network…", danger: true }, danger: false };
const PLAIN = { title: "Block?", body: ["x"], action: "Block", danger: false };

let wrapper;
afterEach(() => wrapper.unmount());

function setup() {
  wrapper = mount(ConfirmDialog, { attachTo: document.body });
  const q = (id) => wrapper.find(id);
  return {
    ask: async (spec) => {
      const answer = wrapper.vm.ask(spec);
      await flushPromises();
      return answer;
    },
    type: async (text) => q("#member-reason").setValue(text),
    yes: () => q("#member-confirm-yes"),
    alt: () => q("#member-confirm-alt"),
  };
}

describe("the typed network name", () => {
  it("keeps the main button disabled until the name matches", async () => {
    const page = setup();
    void page.ask(DELETE);
    await flushPromises();
    expect(page.yes().element.disabled).toBe(true);
    await page.type("wor");
    expect(page.yes().element.disabled).toBe(true);
    await page.type("work");
    expect(page.yes().element.disabled).toBe(false);
  });

  it("ignores spaces around the typed name", async () => {
    const page = setup();
    void page.ask(DELETE);
    await flushPromises();
    await page.type("  work ");
    expect(page.yes().element.disabled).toBe(false);
  });

  it("starts locked on every opening", async () => {
    const page = setup();
    void page.ask(DELETE);
    await flushPromises();
    await page.type("work");
    await page.yes().trigger("click");
    await flushPromises();
    void page.ask(DELETE);
    await flushPromises();
    expect(page.yes().element.disabled).toBe(true);
  });

  it("a field without `match` never locks the button", async () => {
    const page = setup();
    void page.ask({ ...DELETE, field: { label: "Reason", max: 80 } });
    await flushPromises();
    expect(page.yes().element.disabled).toBe(false);
  });
});

describe("the second action button", () => {
  it("is drawn for a spec with `alt`, red when asked", async () => {
    const page = setup();
    void page.ask(LAST);
    await flushPromises();
    expect(page.alt().text()).toBe("Delete network…");
    expect(page.alt().classes()).toContain("btn-danger");
  });

  it("resolves false and tells it was the alternative", async () => {
    const page = setup();
    const answer = page.ask(LAST);
    await flushPromises();
    await page.alt().trigger("click");
    expect(await answer).toBe(false);
    expect(wrapper.vm.alt()).toBe(true);
  });

  it("the main button is not the alternative", async () => {
    const page = setup();
    const answer = page.ask(LAST);
    await flushPromises();
    await page.yes().trigger("click");
    expect(await answer).toBe(true);
    expect(wrapper.vm.alt()).toBe(false);
  });

  it("a spec without `alt` draws no such button", async () => {
    const page = setup();
    void page.ask(PLAIN);
    await flushPromises();
    expect(page.alt().exists()).toBe(false);
  });
});
