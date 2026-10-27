/* The setup wizard shell and its join and exit screens: the title and the region, the first screen without a network
 * (the join and create forms), the exit screen. Moving between screens is in `SetupWizard.flow.vtest.mjs`. */

import { nextTick } from "vue";
import { afterEach, describe, expect, it } from "vitest";
import { baseState } from "../testing/section.mjs";
import { mountWizard, step, unmountWizard } from "../testing/wizard.mjs";

const REGION = '[role="region"][aria-labelledby="home-wizard-title"]';
let mounted;
const mount = (opts) => (mounted = mountWizard(opts));

afterEach(() => unmountWizard(mounted));

describe("SetupWizard > the shell", () => {
  it("a region titled \"Set up Mesh Town\" holds the screen", () => {
    const { wrapper } = mount({ mark: "browser" });
    expect(wrapper.get(REGION).get("#home-wizard-title").text()).toBe("Set up Mesh Town");
    expect(wrapper.get(REGION).find("#wizard-screen").exists()).toBe(true);
  });

  it("no list of steps: neither the old step buttons nor the badges", () => {
    const { wrapper } = mount({ mark: "browser" });
    expect(wrapper.find(".step-num").exists()).toBe(false);
    expect(wrapper.find("#wizard-step-2").exists()).toBe(false);
    expect(wrapper.find("#wizard-step-3").exists()).toBe(false);
  });

  it("in a network the join screen is gone and the invite sits under the setup", () => {
    const { wrapper } = mount({ mark: "browser" });
    expect(wrapper.find("#home-start-choice").exists()).toBe(false);
    expect(wrapper.find("#pac-row").exists(), "the address is in the guide, not in the question").toBe(false);
    expect(wrapper.find("#netkey-invite-btn").exists()).toBe(true);
    expect(wrapper.get(REGION).find("#netkey-invite-btn").exists(), "outside the setup region").toBe(false);
  });
});

describe("SetupWizard > join screen, no network", () => {
  it("first screen: the \"You haven't joined\" panel, two buttons, no form, no navigation", () => {
    const { wrapper } = mount({ hasNet: false });
    expect(step(wrapper)).toBe("join");
    expect(wrapper.get("#home-start-title").text()).toBe("You haven't joined a Mesh Town network yet");
    expect(wrapper.get("#home-start-join-btn").text()).toBe("Join with an invite");
    expect(wrapper.get("#home-start-create-btn").text()).toBe("Create a new network");
    expect(wrapper.find("#join-card").exists()).toBe(false);
    expect(wrapper.find("#wizard-joined").exists()).toBe(false);
    expect(wrapper.find(".wizard-nav").exists()).toBe(false);
    expect(wrapper.find("#netkey-invite-btn").exists(), "nowhere to invite").toBe(false);
    expect(wrapper.find("#pac-row").exists(), "the browser is connected after joining the network").toBe(false);
  });

  it("\"Join with an invite\": the invite field is visible, no creation, focus in the field", async () => {
    const { wrapper } = mount({ hasNet: false });
    const btn = wrapper.get("#home-start-join-btn");
    await btn.trigger("click");
    await nextTick();
    expect(wrapper.get("#join-box").attributes("hidden")).toBeUndefined();
    expect(wrapper.get("#create-box").attributes("hidden")).toBeDefined();
    expect(wrapper.find("#create-toggle").exists()).toBe(false);
    expect(document.activeElement).toBe(wrapper.get("#join-key-input").element);
    expect(btn.attributes("aria-expanded")).toBe("true");
  });

  it("\"Create a new network\": the name is visible, no invite; \"Join…\" swaps the forms", async () => {
    const { wrapper } = mount({ hasNet: false });
    await wrapper.get("#home-start-create-btn").trigger("click");
    await nextTick();
    expect(wrapper.get("#create-box").attributes("hidden")).toBeUndefined();
    expect(wrapper.get("#join-box").attributes("hidden")).toBeDefined();
    expect(document.activeElement).toBe(wrapper.get("#create-label-input").element);
    await wrapper.get("#home-start-join-btn").trigger("click");
    expect(wrapper.get("#join-box").attributes("hidden")).toBeUndefined();
    expect(wrapper.get("#create-box").attributes("hidden")).toBeDefined();
  });
});

describe("SetupWizard > join screen > primary button", () => {
  it("join button stays primary: before a choice", () => {
    const { wrapper } = mount({ hasNet: false });
    expect(wrapper.get("#home-start-join-btn").classes()).toContain("btn-primary");
  });

  it("after \"Create a new network\" the primary one is \"Create\" in the form", async () => {
    const { wrapper } = mount({ hasNet: false });
    await wrapper.get("#home-start-create-btn").trigger("click");
    await nextTick();
    expect(wrapper.get("#home-start-join-btn").classes(), "join button stays primary: after Create").toContain("btn-primary");
    expect(wrapper.get("#create-btn").classes()).toContain("btn-primary");
  });

  it("after \"Join…\" the primary one is \"Join\" in the form", async () => {
    const { wrapper } = mount({ hasNet: false });
    await wrapper.get("#home-start-join-btn").trigger("click");
    await nextTick();
    expect(wrapper.get("#join-btn").classes()).toContain("btn-primary");
    expect(wrapper.get("#home-start-join-btn").classes(), "join button stays primary: after Join").toContain("btn-primary");
    expect(wrapper.get("#home-start-join-btn").attributes("aria-expanded")).toBe("true");
  });
});

describe("SetupWizard > exit screen", () => {
  it("the question \"Choose an exit\" and the exit phrase under it", () => {
    const { wrapper } = mount({ mark: "exit" });
    expect(step(wrapper)).toBe("exit");
    expect(wrapper.get("#wizard-q").text()).toBe("Choose an exit");
    expect(wrapper.get("#exit-explain").text()).toContain("An exit is a member whose internet");
  });

  it("the address and the manual setup are not on this screen", () => {
    const { wrapper } = mount({ mark: "exit" });
    expect(wrapper.find("#pac-row").exists()).toBe(false);
    expect(wrapper.find("#manual-toggle").exists()).toBe(false);
  });

  it("the name of the computer is on the screen under \"This computer\"; no phone card, no \"Tap Connect\"", () => {
    const { wrapper } = mount({ mark: "exit" });
    expect(wrapper.get("#wizard-screen h4").text()).toBe("This computer");
    expect(wrapper.get(REGION).find("#field-node-name").exists()).toBe(true);
    expect(wrapper.get("label[for=field-node-name]").text()).toBe("This computer's name");
    expect(wrapper.find("#home-phone-card").exists()).toBe(false);
    expect(wrapper.find("#wizard-connect-step").exists()).toBe(false);
    expect(wrapper.text()).not.toContain("Tap Connect");
  });

  it("the name field has no \"Save\" button, \"Finish\" is the last one", () => {
    const { wrapper } = mount({ mark: "exit" });
    const texts = wrapper.get(REGION).findAll("button").map((b) => b.text());
    expect(texts).not.toContain("Save");
    expect(texts).toContain("Finish");
    expect(texts).not.toContain("Done");
  });

  it("joined but alone: a spinner and \"Looking for other members\"; with members — the usual choice", () => {
    const self = baseState().peers.filter((p) => p.self);
    const alone = mount({ mark: "exit", over: { peers: self } });
    expect(alone.wrapper.get("#exit-searching .spinner").exists()).toBe(true);
    expect(alone.wrapper.get("#exit-searching").text()).toContain("Looking for other members");
    unmountWizard(alone);
    const { wrapper } = mount({ mark: "exit" });
    expect(wrapper.find("#exit-searching").exists()).toBe(false);
    expect(wrapper.find(".exit-choice").exists()).toBe(true);
  });
});
