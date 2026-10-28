/* The setup wizard on a router: the path is system, instruction; the question is which device is connected, the
 * address is the router's, and a closed proxy shows no address. */

import { afterEach, describe, expect, it } from "vitest";
import { netsConfig, routerState } from "../testing/nets-states.mjs";
import { click, cursor, mountWizard, plain, rows, unmountWizard } from "../testing/wizard.mjs";
import { WIZ_Q_DEVICE } from "./wizard/wizard-texts.ts";

let mounted;
const ROUTER_PAC = "http://192.168.88.1:1080/proxy.pac";

function mountRouter(over) {
  const state = routerState(over);
  mounted = mountWizard({ mark: "browser", state, config: netsConfig(state) });
  return mounted.wrapper;
}

afterEach(() => unmountWizard(mounted));

describe("SetupWizard > router > open for the home network", () => {
  it("asks which device is connected, with no scope or browser question", async () => {
    const wrapper = mountRouter({ access: "localnet" });
    expect(rows(wrapper)).toEqual(["os", "guide"]);
    expect(wrapper.get("#wizard-q-os").text()).toBe(WIZ_Q_DEVICE);
    expect(wrapper.find("#scope-browser").exists()).toBe(false);
    expect(cursor(wrapper)).toBe("guide");
  });

  it("shows the router's address and the networks summary", async () => {
    const wrapper = mountRouter({ access: "localnet" });
    expect(wrapper.get("#pac-url-text").text()).toBe(ROUTER_PAC);
    expect(wrapper.find("#nets-wizard").exists()).toBe(true);
  });

  it("the iPhone tile switches the instruction to the iPhone", async () => {
    const wrapper = mountRouter({ access: "localnet" });
    await click(wrapper, "#os-ios");
    expect(wrapper.get("#wizard-q-guide").text()).toBe("Connect an iPhone or iPad to the router");
    expect(plain(wrapper.get("#pac-steps"))).toContain("Configure Proxy");
  });
});

describe("SetupWizard > router > closed to the home network", () => {
  it("says the proxy is for the router only and hides the address", async () => {
    const wrapper = mountRouter({ access: "localhost" });
    expect(wrapper.find("#router-only").exists()).toBe(true);
    expect(wrapper.find("#pac-url-text").exists()).toBe(false);
    expect(wrapper.find("#pac-copy-btn").exists()).toBe(false);
  });
});
