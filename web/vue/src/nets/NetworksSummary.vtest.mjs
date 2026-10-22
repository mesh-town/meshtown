/* Summary in step 2 of the setup on a router (`NetworksSummary.vue`): "The proxy is open for…", "Edit" expands the networks block
 * in place, the "router only" mode with the "Open for the home network" button. The node is `createFakeNode`. */

import { afterEach, describe, expect, it } from "vitest";
import { flat, lastPut, mountWithNode, settled } from "../testing/nets-mount.mjs";
import { routerGuestState, routerState } from "../testing/nets-states.mjs";
import NetworksSummary from "./NetworksSummary.vue";
import { ROUTER_ONLY_TEXT, SAVED_NOTE } from "./nets-texts.ts";

let mounted;
afterEach(() => mounted?.wrapper.unmount());

describe("NetworksSummary", () => {
  describe("proxy open for a network (socks.access=localnet)", () => {
    it("automatic: \"The proxy is open for: Home network. The ISP network is closed automatically.\"", () => {
      const { wrapper } = setup(routerState());
      expect(flat(wrapper.get("#nets-summary"))).toContain("The proxy is open for: Home network. The ISP network is closed automatically.");
      expect(wrapper.get("#nets-summary b").text()).toBe("Home network");
    });

    it("list set: \"Closed for: Guest network, ISP network.\"", () => {
      const { wrapper } = setup(routerGuestState());
      expect(flat(wrapper.get("#nets-summary"))).toContain("The proxy is open for: Home network. Closed for: Guest network, ISP network.");
    });

    it("\"Edit\" expands the networks block in place, a second click collapses it; aria-expanded follows", async () => {
      const { wrapper } = setup(routerState());
      const edit = wrapper.get("#nets-edit-btn");
      expect(edit.text()).toBe("Edit");
      expect(edit.attributes("aria-expanded")).toBe("false");
      expect(wrapper.find("#nets-block").exists()).toBe(false);
      await edit.trigger("click");
      expect(wrapper.get("#nets-edit-btn").attributes("aria-expanded")).toBe("true");
      expect(wrapper.find("#nets-block").exists()).toBe(true);
      await wrapper.get("#nets-edit-btn").trigger("click");
      expect(wrapper.find("#nets-block").exists()).toBe(false);
    });

    it("after \"Save\" the block collapses, the summary is rebuilt from the saved list, \"Saved…\" below", async () => {
      const { wrapper, node } = setup(routerState());
      await wrapper.get("#nets-edit-btn").trigger("click");
      await wrapper.get("#net-eth0").setValue(true);
      await wrapper.get("#nets-save").trigger("click");
      await settled();
      expect(lastPut(node).config.socks.ifaces).toEqual(["br-lan", "eth0"]);
      expect(wrapper.find("#nets-block").exists(), "the block collapsed").toBe(false);
      expect(flat(wrapper.get("#nets-summary"))).toContain("The proxy is open for: Home network, ISP network.");
      expect(flat(wrapper.get("#nets-summary"))).not.toContain("closed automatically");
      expect(wrapper.get("#nets-saved-note").text()).toBe(SAVED_NOTE);
    });
  });

  describe("proxy only for this computer (socks.access=localhost)", () => {
    it("instead of the summary — a text about the router and the \"Open for the home network\" button", () => {
      const { wrapper } = setup(routerState({ access: "localhost" }));
      expect(wrapper.find("#nets-summary").exists()).toBe(false);
      expect(wrapper.get("#router-only").text()).toContain(ROUTER_ONLY_TEXT);
      expect(wrapper.get("#nets-open-btn").text()).toBe("Open for the home network");
    });

    it("the button writes socks.access=localnet and shows \"Saved. Takes effect after Mesh Town restarts.\"", async () => {
      const { wrapper, node } = setup(routerState({ access: "localhost" }));
      await wrapper.get("#nets-open-btn").trigger("click");
      await settled();
      expect(lastPut(node).config.socks.access).toBe("localnet");
      expect(wrapper.get("#nets-saved-note").text()).toBe(SAVED_NOTE);
    });
  });
});

function setup(state) {
  return (mounted = mountWithNode(NetworksSummary, state));
}
