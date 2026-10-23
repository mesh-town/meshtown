/* "Settings" with network selection: where the "Which local networks can connect" block sits
 * (under "Who can use the proxy", in the "Page" section, or nowhere), the field description, the "From the same networks" line and
 * the page access options without the word localhost. The rest is in `SettingsSection.vtest.mjs`. */

import { afterEach, describe, expect, it, vi } from "vitest";
import { DESC_SOCKS_ACCESS_NETS } from "../lan/nets-texts.ts";
import { flat, lastPut, mountWithNode, settled } from "../testing/nets-mount.mjs";
import { laptopState, routerState } from "../testing/nets-states.mjs";
import SettingsSection from "./SettingsSection.vue";

const OLD_DESC = "any device on your network (and guest Wi-Fi, if it isn't separated)";
let mounted;
afterEach(() => mounted?.wrapper.unmount());

describe("SettingsSection > network selection", () => {
  describe("two networks, proxy for the local network", () => {
    it("the block sits under the \"Who can use the proxy\" field, in the main settings, not in the \"Page\" section", () => {
      const { wrapper } = setup(routerState());
      const field = wrapper.get("#field-socks-access").element;
      const block = wrapper.get("#nets-block").element;
      expect(field.compareDocumentPosition(block) & Node.DOCUMENT_POSITION_FOLLOWING, "block after the field").toBeTruthy();
      expect(wrapper.find("details #nets-block").exists(), "not inside \"Advanced\"").toBe(false);
      expect(wrapper.findAll("#nets-block")).toHaveLength(1);
    });

    it("the field description is about \"the networks checked below\"", () => {
      const text = flat(setup(routerState()).wrapper);
      expect(text).toContain(DESC_SOCKS_ACCESS_NETS);
      expect(text).not.toContain(OLD_DESC);
    });

    it("the page is also for the local network: the \"Page\" section has a \"From the same networks as the proxy\" line with a \"change\" link", () => {
      const { wrapper } = setup(routerState());
      expect(wrapper.get("#same-nets").text()).toContain("From the same networks as the proxy: Home network.");
      expect(wrapper.get("#same-nets-link").text()).toBe("change");
    });

    it("the \"change\" link scrolls the page to the networks block", async () => {
      const scroll = vi.fn();
      Element.prototype.scrollIntoView = scroll;
      const { wrapper } = setup(routerState());
      await wrapper.get("#same-nets-link").trigger("click");
      expect(scroll).toHaveBeenCalledTimes(1);
      expect(scroll.mock.contexts[0].id, "scroll at the networks block").toBe("nets-block");
    });

    it("saving the block in \"Settings\": the section's bottom bar gives a restart button, the block has no second button", async () => {
      const { wrapper, node } = setup(routerState());
      await wrapper.get("#net-eth0").setValue(true);
      await wrapper.get("#nets-save").trigger("click");
      await settled();
      expect(lastPut(node).config.socks.ifaces).toEqual(["br-lan", "eth0"]);
      expect(wrapper.find("#nets-restart-btn").exists()).toBe(false);
      expect(wrapper.find("#restart-node-btn").exists(), "bottom bar button").toBe(true);
    });
  });

  describe("proxy for this computer only, page for the local network", () => {
    it("the block sits in the \"Page\" section (Advanced), there is no \"From the same networks\" line", () => {
      const { wrapper } = setup(routerState({ access: "localhost" }));
      expect(wrapper.find("details #nets-block").exists()).toBe(true);
      expect(wrapper.findAll("#nets-block")).toHaveLength(1);
      expect(wrapper.find("#same-nets").exists()).toBe(false);
    });
  });

  describe("no networks block", () => {
    it("on a laptop with one network — no block and no line, the field description is unchanged", () => {
      const { wrapper } = setup(laptopState({ access: "localnet" }));
      expect(wrapper.find("#nets-block").exists()).toBe(false);
      expect(wrapper.find("#same-nets").exists()).toBe(false);
      expect(flat(wrapper)).toContain(OLD_DESC);
    });

    it("on a router where both the proxy and the page are \"this computer only\"", () => {
      const { wrapper } = setup(routerState({ access: "localhost", webAccess: "localhost" }));
      expect(wrapper.find("#nets-block").exists()).toBe(false);
    });
  });

  describe("page access options", () => {
    it("without the word localhost", () => {
      const { wrapper } = setup(routerState());
      expect(wrapper.text()).not.toContain("(localhost)");
      expect(flat(wrapper)).toContain("From this computer and the local network");
    });
  });
});

function setup(state) {
  return (mounted = mountWithNode(SettingsSection, state, { configFile: { writable: true } }));
}
