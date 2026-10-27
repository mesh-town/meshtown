/* `members/ExcludedCard.vue` inside the Android app (the user agent carries `MeshTownApp/`): the same card as in a browser,
 * with the wording about the phone. */

import { afterEach, describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import ExcludedCard from "./ExcludedCard.vue";

let wrapper;
afterEach(() => {
  wrapper.unmount();
  Reflect.deleteProperty(navigator, "userAgent");
});

describe("ExcludedCard in the app", () => {
  it.each(["banned", "left"])("%s: the join button, no reset link", (variant) => {
    setup(variant);
    expect(wrapper.get("#excluded-join").text()).toBe("Join with another invite");
    expect(wrapper.find("#excluded-start-over").exists()).toBe(false);
    expect(wrapper.find("a").exists()).toBe(false);
  });

  it.each(["banned", "left"])("%s: the text never says computer and offers no reset", (variant) => {
    setup(variant);
    const text = wrapper.get("#excluded-card").text();
    expect(text).not.toContain("computer");
    expect(text).not.toContain("reset");
    expect(text).not.toContain("Leave network");
  });

  it("banned: the text says who can help and that a new invite is needed", () => {
    setup("banned");
    expect(wrapper.get("#excluded-card p").text()).toBe(
      "An admin removed it. Sites no longer open through Mesh Town, and members can't reach this phone. If you think this is a mistake, ask an admin of the network for a new invite.",
    );
  });

  it("the button reports the click to the parent", async () => {
    setup("banned");
    await wrapper.get("#excluded-join").trigger("click");
    expect(wrapper.emitted("join")).toHaveLength(1);
  });

  it.each(["banned", "left"])("%s: the title names the phone, not the computer", (variant) => {
    setup(variant);
    const title = wrapper.get("h2").text();
    expect(title).not.toContain("computer");
    if (variant !== "left") expect(title).toContain("This phone was");
  });

  it("with an admin named: the app shows the join button and the title names the phone", () => {
    setup("banned", undefined, "alice");
    expect(wrapper.find("#excluded-join").exists()).toBe(true);
    expect(wrapper.find("#excluded-start-over").exists()).toBe(false);
    expect(wrapper.get("h2").text()).toContain("This phone was");
  });

  it("outside the app the card is the same, with the word computer", () => {
    setup("banned", "Mozilla/5.0 Chrome/130.0");
    expect(wrapper.get("#excluded-join").text()).toBe("Join with another invite");
    expect(wrapper.get("#excluded-card p").text()).toContain("this computer");
  });
});

function setup(variant, userAgent = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android", by = null) {
  Object.defineProperty(navigator, "userAgent", { value: userAgent, configurable: true });
  wrapper = mount(ExcludedCard, { props: { variant, net: "demo-network", by }, attachTo: document.body });
}
