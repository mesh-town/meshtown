/* `members/ExcludedCard.vue` inside the Android app (the user agent carries `MeshTownApp/`): "Leave network and reset…" (the
 * title of the app's own menu item) replaces "Join with another invite" and leads to the address the app intercepts. */

import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import ExcludedCard from "./ExcludedCard.vue";

let wrapper;
afterEach(() => {
  wrapper.unmount();
  Reflect.deleteProperty(navigator, "userAgent");
});

describe("ExcludedCard in the app", () => {
  it.each(["banned", "left"])("%s: a \"Leave network and reset…\" link to the app address, no join button", (variant) => {
    setup(variant);
    const link = wrapper.get("#excluded-start-over");
    expect(link.text()).toBe("Leave network and reset\u2026");
    expect(link.attributes("href")).toBe("meshtown-app://start-over");
    expect(wrapper.find("#excluded-join").exists()).toBe(false);
  });

  it.each(["banned", "left"])("%s: explains that the phone was banned and what to do — leave and reset the app", (variant) => {
    setup(variant);
    const text = wrapper.get("#excluded-card").text();
    expect(text).toContain("This phone was banned from the network. Leave the network and reset the app to join with a new invite.");
    expect(text).not.toContain("Start over");
  });

  it("forgotten: no reset link, the ordinary text and the join button stay", () => {
    setup("forgotten");
    expect(wrapper.find("#excluded-start-over").exists()).toBe(false);
    expect(wrapper.find("#excluded-join").exists()).toBe(true);
    expect(wrapper.get("#excluded-card").text()).not.toContain("reset the app");
  });

  it.each(["banned", "left", "forgotten"])("%s: the title names the phone, not the computer", (variant) => {
    setup(variant);
    const title = wrapper.get("h2").text();
    expect(title).not.toContain("computer");
    if (variant !== "left") expect(title).toContain("This phone was");
  });

  it("forgotten with an admin: the text says the admin removed this phone", () => {
    setup("forgotten", undefined, "alice");
    const text = wrapper.get("#excluded-card p").text();
    expect(text).toContain("removed this phone");
    expect(text).not.toContain("computer");
  });

  it("forgotten: the explanation says phone", () => {
    setup("forgotten");
    expect(wrapper.get("#excluded-card p").text()).not.toContain("computer");
  });

  it("outside the app the join button stays", () => {
    setup("banned", "Mozilla/5.0 Chrome/130.0");
    expect(wrapper.find("#excluded-start-over").exists()).toBe(false);
    expect(wrapper.get("#excluded-join").text()).toBe("Join with another invite");
  });
});

function setup(variant, userAgent = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android", by = null) {
  Object.defineProperty(navigator, "userAgent", { value: userAgent, configurable: true });
  wrapper = mount(ExcludedCard, { props: { variant, net: "demo-network", by }, attachTo: document.body });
}
