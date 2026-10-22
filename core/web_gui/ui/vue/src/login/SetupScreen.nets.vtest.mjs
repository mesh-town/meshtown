/* "Set a password" screen: page access options without the word localhost. There are no networks on this
 * screen, neither on a laptop nor on a router. The rest is in `SetupScreen.vtest.mjs`. */

import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { STORE_KEY, createStore } from "../core/store.ts";
import { baseConfig } from "../testing/section.mjs";
import SetupScreen from "./SetupScreen.vue";

let wrapper;
afterEach(() => wrapper?.unmount());

describe("SetupScreen > page access", () => {
  it("options: \"From this computer only\" and \"From this computer and the local network\"", () => {
    const texts = mountSetup().findAll("#setup-access option").map((o) => o.text());
    expect([...texts].sort()).toEqual(["From this computer only", "From this computer and the local network"].sort());
  });

  it("option values are unchanged: localhost and localnet", () => {
    expect(mountSetup().findAll("#setup-access option").map((o) => o.attributes("value")).sort()).toEqual(["localhost", "localnet"]);
  });

  it("the word localhost is not in the visible text of the screen", () => {
    expect(mountSetup().text()).not.toMatch(/localhost/i);
  });

  it("there is no network selection block on the screen", () => {
    expect(mountSetup().find("#nets-block").exists()).toBe(false);
  });
});

function mountSetup() {
  const store = createStore();
  store.config = baseConfig();
  store.configRev = 1;
  wrapper = mount(SetupScreen, { global: { provide: { [STORE_KEY]: store } } });
  return wrapper;
}
