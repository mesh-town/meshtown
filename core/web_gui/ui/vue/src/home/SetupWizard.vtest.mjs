/* The "Three steps" setup: which steps are active, the "Done" buttons set a mark in localStorage. */

import { nextTick } from "vue";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { baseState, mountSection } from "../testing/section.mjs";
import SetupWizard from "./SetupWizard.vue";

const REGION = '[role="region"][aria-labelledby="home-wizard-title"]';
const MARK = "meshtown.homeSetup";
const WITH_PROXY = { socks: { enabled: true, listen: "127.0.0.1:1080" } };
let mounted;

beforeEach(() => {
  localStorage.clear();
  localStorage.setItem("meshtown.proxyScope", "system"); // the scope question is answered; its own tests are in BrowserCard.scope.vtest.mjs
});
afterEach(() => mounted.wrapper.unmount());

describe("SetupWizard > step 1, no network", () => {
  it("first screen: the \"You haven't joined\" panel, two buttons, no form, steps 2 and 3 wait", () => {
    const { wrapper } = mountWizard(baseState({ network: null, peers: [] }));
    const panel = wrapper.get("#home-start-choice");
    expect(panel.attributes("role")).toBe("region");
    expect(wrapper.get("#home-start-title").text()).toBe("You haven't joined a Mesh Town network yet");
    expect(wrapper.get("#home-start-join-btn").text()).toBe("Join with an invite");
    expect(wrapper.get("#home-start-create-btn").text()).toBe("Create a new network");
    expect(wrapper.find("#join-card").exists()).toBe(false);
    expect(wrapper.find(REGION).exists()).toBe(true);
    expect(wrapper.findAll(".wizard-step.pending")).toHaveLength(2);
    expect(wrapper.find("#netkey-invite-btn").exists(), "nowhere to invite").toBe(false);
    expect(wrapper.find("#pac-row").exists(), "the browser is connected after joining the network").toBe(false);
  });

  it("\"Join with an invite\": the invite field is visible, no creation, focus in the field", async () => {
    const { wrapper } = mountWizard(baseState({ network: null, peers: [] }));
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
    const { wrapper } = mountWizard(baseState({ network: null, peers: [] }));
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

describe("SetupWizard > primary button", () => {
  it("before a choice the primary one is \"Join…\"", () => {
    const { wrapper } = mountWizard(baseState({ network: null, peers: [] }));
    expect(wrapper.get("#home-start-join-btn").classes()).toContain("btn-primary");
  });

  it("after \"Create a new network\" the primary one is \"Create\" in the form", async () => {
    const { wrapper } = mountWizard(baseState({ network: null, peers: [] }));
    await wrapper.get("#home-start-create-btn").trigger("click");
    await nextTick();
    expect(wrapper.get("#home-start-join-btn").classes()).not.toContain("btn-primary");
    expect(wrapper.get("#create-btn").classes()).toContain("btn-primary");
  });

  it("after \"Join…\" the primary one is \"Join\" in the form", async () => {
    const { wrapper } = mountWizard(baseState({ network: null, peers: [] }));
    await wrapper.get("#home-start-join-btn").trigger("click");
    await nextTick();
    expect(wrapper.get("#join-btn").classes()).toContain("btn-primary");
    expect(wrapper.get("#home-start-join-btn").classes()).not.toContain("btn-primary");
  });
});

describe("SetupWizard > step 2, mark \"browser\"", () => {
  it("Z1 absent, \"You joined the network\", the browser steps, the invite under the setup", () => {
    const { wrapper } = mountWizard(baseState(WITH_PROXY), "browser");
    expect(wrapper.find("#home-start-choice").exists()).toBe(false);
    expect(wrapper.text()).toContain("✓ You joined the network");
    expect(wrapper.find("#pac-row").exists()).toBe(true);
    expect(wrapper.find("#pac-copy-btn").exists()).toBe(true);
    expect(wrapper.find("#manual-toggle").exists()).toBe(true);
    expect(wrapper.find("#netkey-invite-btn").exists()).toBe(true);
  });

  it("the PAC address is built from socks.listen, the manual steps are collapsed, and \"Not working?\" expands them", async () => {
    const { wrapper } = mountWizard(baseState(WITH_PROXY), "browser");
    expect(wrapper.get("#pac-url-text").text()).toBe("http://127.0.0.1:1080/proxy.pac");
    expect(wrapper.find("#proxy-setup").exists(), "before the click on #manual-toggle").toBe(false);
    await wrapper.get("#manual-toggle").trigger("click");
    expect(wrapper.find("#proxy-setup").exists()).toBe(true);
    expect(wrapper.find("#socks-copy-port-btn").exists()).toBe(true);
  });

  it("step 2 carries the strict-mode question under the browser card, before the Done button", () => {
    const { wrapper } = mountWizard(baseState(WITH_PROXY), "browser");
    const html = wrapper.html();
    expect(wrapper.find("#strict-choice").exists()).toBe(true);
    expect(html.indexOf("strict-choice")).toBeGreaterThan(html.indexOf("pac-row"));
    expect(html.indexOf("strict-choice")).toBeLessThan(html.indexOf("Done, my browser is set up"));
    expect(wrapper.get("#wizard-step-2").text()).toBe("2Set up your browser");
  });

  it("\"Done, my browser is set up\" sets the exit mark and opens step 3", async () => {
    const { wrapper } = mountWizard(baseState(WITH_PROXY), "browser");
    const done = wrapper.findAll("button").find((b) => b.text() === "Done, my browser is set up");
    await done.trigger("click");
    expect(localStorage.getItem(MARK)).toBe("exit");
    expect(wrapper.find("#home-exit").exists()).toBe(true);
    expect(wrapper.find("#field-node-name").exists()).toBe(true);
  });
});

describe("SetupWizard > step 3, mark \"exit\"", () => {
  it("the title \"Choose an exit\" and the exit phrase under it", () => {
    const { wrapper } = mountWizard(baseState(WITH_PROXY), "exit");
    expect(wrapper.text()).toContain("Choose an exit");
    expect(wrapper.get("#exit-explain").text()).toContain("An exit is a member whose internet");
  });

  it("the PAC address line is gone — it is only in step 2", () => {
    const { wrapper } = mountWizard(baseState(WITH_PROXY), "exit");
    expect(wrapper.find("#pac-row").exists()).toBe(false);
    expect(wrapper.find("#manual-toggle").exists()).toBe(false);
  });

  it("\"Done\" sets the done mark", async () => {
    const { wrapper } = mountWizard(baseState(WITH_PROXY), "exit");
    const done = wrapper.findAll("button").find((b) => b.text() === "Done");
    await done.trigger("click");
    expect(localStorage.getItem(MARK)).toBe("done");
  });
});

describe("SetupWizard > choosing a step by clicking the title", () => {
  it("without a network the titles of steps 2 and 3 are not clickable", () => {
    const { wrapper } = mountWizard(baseState({ network: null, peers: [] }));
    expect(wrapper.get("#wizard-step-2").attributes("disabled")).toBeDefined();
    expect(wrapper.get("#wizard-step-3").attributes("disabled")).toBeDefined();
  });

  it("in a network: a click on step 3 and back on step 2 changes the step, doesn't touch the mark", async () => {
    const { wrapper } = mountWizard(baseState(WITH_PROXY), "browser");
    expect(wrapper.get("#wizard-step-2").attributes("aria-current")).toBe("step");
    await wrapper.get("#wizard-step-3").trigger("click");
    expect(wrapper.find("#home-exit").exists()).toBe(true);
    expect(wrapper.find("#pac-row").exists()).toBe(false);
    expect(wrapper.get("#wizard-step-3").attributes("aria-current")).toBe("step");
    await wrapper.get("#wizard-step-2").trigger("click");
    expect(wrapper.find("#pac-row").exists()).toBe(true);
    expect(wrapper.find("#home-exit").exists()).toBe(false);
    expect(localStorage.getItem(MARK)).toBe("browser");
  });

  it("a completed setup: step 3 opens by a click, the done mark stays", async () => {
    const { wrapper } = mountWizard(baseState(WITH_PROXY), "done");
    await wrapper.get("#wizard-step-3").trigger("click");
    expect(wrapper.find("#home-exit").exists()).toBe(true);
    expect(localStorage.getItem(MARK)).toBe("done");
  });

  it("after choosing step 2 \"Done, my browser is set up\" leads to step 3", async () => {
    const { wrapper } = mountWizard(baseState(WITH_PROXY), "exit");
    await wrapper.get("#wizard-step-2").trigger("click");
    const done = wrapper.findAll("button").find((b) => b.text() === "Done, my browser is set up");
    await done.trigger("click");
    expect(wrapper.find("#home-exit").exists()).toBe(true);
    expect(localStorage.getItem(MARK)).toBe("exit");
  });
});

describe("SetupWizard > exit step while looking for members", () => {
  it("joined but alone: a spinner and \"Looking for other members\"; with members — the usual choice", () => {
    const self = baseState().peers.filter((p) => p.self);
    const alone = mountWizard(baseState({ ...WITH_PROXY, peers: self }), "exit");
    expect(alone.wrapper.get("#exit-searching .spinner").exists()).toBe(true);
    expect(alone.wrapper.get("#exit-searching").text()).toContain("Looking for other members");
    alone.wrapper.unmount();
    const { wrapper } = mountWizard(baseState(WITH_PROXY), "exit");
    expect(wrapper.find("#exit-searching").exists()).toBe(false);
    expect(wrapper.find(".exit-choice").exists()).toBe(true);
  });
});

describe("SetupWizard > the browser version stays as it was", () => {
  it("the title is \"Three steps to get started\", step 2 is the browser, step 3 is \"3Choose an exit\"", () => {
    const { wrapper } = mountWizard(baseState(WITH_PROXY), "exit");
    expect(wrapper.get("#home-wizard-title").text()).toBe("Three steps to get started");
    expect(wrapper.get("#wizard-step-2").text()).toBe("2Set up your browser");
    expect(wrapper.get("#wizard-step-3").text()).toBe("3Choose an exit");
  });

  it("the name of the computer is inside step 3, there is no separate phone card and no \"Tap Connect\" row", () => {
    const { wrapper } = mountWizard(baseState(WITH_PROXY), "exit");
    expect(wrapper.get(REGION).find("#field-node-name").exists()).toBe(true);
    expect(wrapper.get("label[for=field-node-name]").text()).toBe("Name of this computer");
    expect(wrapper.find("#home-phone-card").exists()).toBe(false);
    expect(wrapper.find("#wizard-connect-step").exists()).toBe(false);
    expect(wrapper.text()).not.toContain("Tap Connect");
  });

  it("the name field has no \"Save\" button, \"Done\" is still there", () => {
    const { wrapper } = mountWizard(baseState(WITH_PROXY), "exit");
    const texts = wrapper.findAll("button").map((b) => b.text());
    expect(texts).not.toContain("Save");
    expect(texts).toContain("Done");
  });
});

function mountWizard(state, mark) {
  if (mark) localStorage.setItem(MARK, mark);
  return (mounted = mountSection(SetupWizard, { state }));
}
