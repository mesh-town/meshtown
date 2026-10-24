/* `members/ExcludedCard.vue` inside the Android app (the user agent carries `MeshTownApp/`): "Start over" replaces
 * "Join with another invite" and leads to the address the app intercepts. */

import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import ExcludedCard from "./ExcludedCard.vue";

let wrapper;
afterEach(() => {
  wrapper.unmount();
  Reflect.deleteProperty(navigator, "userAgent");
});

describe("ExcludedCard in the app", () => {
  it.each(["banned", "left"])("%s: a Start over link to the app address, no join button", (variant) => {
    setup(variant);
    const link = wrapper.get("#excluded-start-over");
    expect(link.text()).toBe("Start over");
    expect(link.attributes("href")).toBe("meshtown-app://start-over");
    expect(wrapper.find("#excluded-join").exists()).toBe(false);
  });

  it("explains that the phone was banned", () => {
    setup("banned");
    expect(wrapper.get("#excluded-card").text()).toContain("This phone was banned from the network. Start over to join with a new invite.");
  });

  it("outside the app the join button stays", () => {
    setup("banned", "Mozilla/5.0 Chrome/130.0");
    expect(wrapper.find("#excluded-start-over").exists()).toBe(false);
    expect(wrapper.get("#excluded-join").text()).toBe("Join with another invite");
  });
});

function setup(variant, userAgent = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android") {
  Object.defineProperty(navigator, "userAgent", { value: userAgent, configurable: true });
  wrapper = mount(ExcludedCard, { props: { variant, net: "demo-network" }, attachTo: document.body });
}
