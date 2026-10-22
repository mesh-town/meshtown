/* The "Which local networks can connect" block (`NetworksBlock.vue`): network rows per the naming table, badges, warnings
 * and, "Save" / "Return to automatic choice", the caption after saving. The node is `createFakeNode`
 * with snapshots from `testing/nets-states.mjs`; the save goes through the real `save-config.ts` and `api.ts`. */

import { afterEach, describe, expect, it } from "vitest";
import { flat, lastPut, mountWithNode, settled } from "../testing/nets-mount.mjs";
import { laptopVpnState, routerGuestState, routerState } from "../testing/nets-states.mjs";
import { NET_NAME } from "../common/words.ts";
import NetworksBlock from "./NetworksBlock.vue";
import { EMPTY_HINT, HERE_NOTE, NETS_SUB, NETS_TAG, NETS_TITLE, ROUTER_RESTART_NOTE, SAVED_NOTE, WARN_PROVIDER } from "./nets-texts.ts";

let mounted;
afterEach(() => mounted?.wrapper.unmount());

describe("NetworksBlock", () => {
  describe("router, networks chosen automatically", () => {
    it("title, the \"after restart\" tag and the subtitle", () => {
      const { wrapper } = setup(routerState());
      expect(wrapper.get("#nets-title").text()).toContain(NETS_TITLE);
      expect(wrapper.get("#nets-tag").text()).toBe(NETS_TAG);
      expect(wrapper.get("#nets-tag").classes()).toContain("mini-badge-restart");
      expect(wrapper.get("#nets-sub").text()).toBe(NETS_SUB);
    });

    it("two rows: \"Home network\" is checked, \"ISP network\" is not checked and labeled \"closed automatically\"", () => {
      const { wrapper } = setup(routerState());
      expect(rowTitles(wrapper)).toEqual(["Home network", "ISP network"]);
      expect(wrapper.get("#net-br-lan").element.checked).toBe(true);
      expect(wrapper.get("#net-eth0").element.checked).toBe(false);
      expect(rowOf(wrapper, "eth0").get(".net-badge").text()).toBe("closed automatically");
    });

    it("small line: the role explanation and \"name · address\" as code", () => {
      const row = rowOf(setup(routerState()).wrapper, "eth0");
      expect(row.get(".net-note").text()).toContain("The router reaches the internet through this network");
      expect(row.get(".net-note code").text()).toBe("eth0 · 192.168.1.100");
    });

    it("list not set: the caption \"The networks are currently chosen automatically\", no return button", () => {
      const { wrapper } = setup(routerState());
      expect(wrapper.find("#nets-auto-note").exists()).toBe(true);
      expect(wrapper.find("#nets-auto-btn").exists()).toBe(false);
    });

    it("\"Save\" is inactive until the checkboxes change", () => {
      expect(setup(routerState()).wrapper.get("#nets-save").attributes("disabled")).toBeDefined();
    });
  });

  describe("the network the page is opened through", () => {
    it("marked \"you are here now\", the row has the locked class and aria-disabled", () => {
      const { wrapper } = setup(routerState());
      expect(rowOf(wrapper, "br-lan").classes()).toContain("locked");
      expect(rowOf(wrapper, "br-lan").get(".net-badge").text()).toBe("you are here now");
      expect(wrapper.get("#net-br-lan").attributes("aria-disabled")).toBe("true");
    });

    it("clicking it does not uncheck it and shows an explanation", async () => {
      const { wrapper } = setup(routerState());
      expect(wrapper.find("#nets-here-note").exists()).toBe(false);
      await wrapper.get("#net-br-lan").trigger("click");
      expect(wrapper.get("#net-br-lan").element.checked, "the checkbox stayed").toBe(true);
      expect(wrapper.get("#nets-here-note").text()).toBe(HERE_NOTE);
    });
  });

  describe("the \"ISP network\" checkbox", () => {
    it("checked and not saved: a warning under the row and \"Save\" is active", async () => {
      const { wrapper } = setup(routerState());
      await wrapper.get("#net-eth0").setValue(true);
      expect(wrapper.get("#net-warn-eth0").text()).toBe(WARN_PROVIDER);
      expect(wrapper.get("#nets-save").attributes("disabled")).toBeUndefined();
    });

    it("unchecked back: the warning is gone, \"Save\" is inactive again", async () => {
      const { wrapper } = setup(routerState());
      await wrapper.get("#net-eth0").setValue(true);
      await wrapper.get("#net-eth0").setValue(false);
      expect(wrapper.find("#net-warn-eth0").exists()).toBe(false);
      expect(wrapper.get("#nets-save").attributes("disabled")).toBeDefined();
    });

    it("\"Cancel\" restores the saved checkboxes", async () => {
      const { wrapper } = setup(routerState());
      await wrapper.get("#net-eth0").setValue(true);
      await wrapper.get("#nets-cancel").trigger("click");
      expect(wrapper.get("#net-eth0").element.checked).toBe(false);
      expect(wrapper.find("#net-warn-eth0").exists()).toBe(false);
    });
  });

  describe("network list is set (router with a guest network)", () => {
    it("unchecked networks get the \"not selected\" badge, not \"closed automatically\"", () => {
      const { wrapper } = setup(routerGuestState());
      expect(rowOf(wrapper, "br-guest").get(".net-badge").text()).toBe("not selected");
      expect(rowOf(wrapper, "eth0").get(".net-badge").text()).toBe("not selected");
    });

    it("instead of the caption — the \"Return to automatic choice\" button; clicking writes both fields empty, without \"Save\"", async () => {
      const { wrapper, node } = setup(routerGuestState());
      expect(wrapper.find("#nets-auto-note").exists()).toBe(false);
      await wrapper.get("#nets-auto-btn").trigger("click");
      await settled();
      expect(lastPut(node).config.socks.ifaces).toEqual([]);
      expect(lastPut(node).config.web.ifaces).toEqual([]);
    });

    it("a network from the list that is not on the machine now — a \"not here now\" row with only the name", () => {
      const state = routerGuestState({ ifaces: ["br-lan", "br-old"] });
      state.node.networks.push({ name: "br-old", addrs: [], default_route: false, present: false, open: false, by: "list" });
      const row = rowOf(setup(state).wrapper, "br-old");
      expect(row.get(".net-badge").text()).toBe("not here now");
      expect(row.get(".net-note code").text()).toBe("br-old");
    });
  });

  describe("saving", () => {
    it("writes one list to socks.ifaces and web.ifaces, then \"Saved. Takes effect after Mesh Town restarts.\" and the router hint", async () => {
      const { wrapper, node } = setup(routerState());
      await wrapper.get("#net-eth0").setValue(true);
      await wrapper.get("#nets-save").trigger("click");
      await settled();
      expect(lastPut(node).config.socks.ifaces).toEqual(["br-lan", "eth0"]);
      expect(lastPut(node).config.web.ifaces).toEqual(["br-lan", "eth0"]);
      expect(wrapper.get("#nets-saved-note").text()).toBe(SAVED_NOTE);
      expect(wrapper.get("#nets-router-hint").text()).toBe(ROUTER_RESTART_NOTE);
    });

    it("in \"Settings\" there is no own restart button — the bottom bar of the section gives it", async () => {
      const { wrapper } = setup(routerState());
      await wrapper.get("#net-eth0").setValue(true);
      await wrapper.get("#nets-save").trigger("click");
      await settled();
      expect(wrapper.find("#nets-restart-btn").exists()).toBe(false);
    });

    it("node refusal: the field text in #nets-error, no \"Saved\" caption", async () => {
      const { wrapper, node } = setup(routerState());
      node.routes["PUT /api/config"] = { status: 422, body: { error: "invalid", message: "not allowed", fields: [{ path: "socks.ifaces", message: "This list would close your page" }] } };
      await wrapper.get("#net-eth0").setValue(true);
      await wrapper.get("#nets-save").trigger("click");
      await settled();
      expect(wrapper.get("#nets-error").text()).toBe("This list would close your page");
      expect(wrapper.get("#nets-error").classes()).toContain("field-error-text");
      expect(wrapper.find("#nets-saved-note").exists()).toBe(false);
    });
  });

  describe("setup (context=wizard)", () => {
    it("after saving, its own \"Restart Mesh Town\" button; clicking sends POST and shows \"Mesh Town is restarting…\", then \"Mesh Town has restarted\"", async () => {
      const { wrapper, node, store } = setup(routerState(), "wizard");
      await wrapper.get("#net-eth0").setValue(true);
      await wrapper.get("#nets-save").trigger("click");
      await settled();
      expect(wrapper.get("#nets-restart-btn").text()).toBe("Restart Mesh Town");
      await wrapper.get("#nets-restart-btn").trigger("click");
      await settled();
      expect(node.count("POST", "/api/actions/restart")).toBe(1);
      expect(wrapper.get("#nets-plate").text()).toBe(NET_NAME + " is restarting…");
      store.configMeta = { ...store.configMeta, restart_pending: [] }; // as after a poll: the node is up, the wait is cleared
      await settled();
      expect(wrapper.get("#nets-plate").text()).toBe(NET_NAME + " has restarted");
    });
  });

  describe("laptop with two networks (en0 and VPN)", () => {
    it("names \"Home network\" and \"VPN connection\", both checked, no \"closed automatically\" badges and no lock", () => {
      const { wrapper } = setup(laptopVpnState({ access: "localnet" }));
      expect(rowTitles(wrapper)).toEqual(["Home network", "VPN connection"]);
      expect(wrapper.get("#net-en0").element.checked).toBe(true);
      expect(wrapper.get("#net-utun3").element.checked).toBe(true);
      expect(wrapper.find(".net-row.locked").exists()).toBe(false);
      expect(flat(wrapper)).not.toContain("closed automatically");
    });

    it("all checkboxes cleared: \"Select at least one network.\" and \"Save\" is inactive", async () => {
      const { wrapper } = setup(laptopVpnState({ access: "localnet" }));
      expect(wrapper.find("#nets-empty-hint").exists()).toBe(false);
      await wrapper.get("#net-en0").setValue(false);
      await wrapper.get("#net-utun3").setValue(false);
      expect(wrapper.get("#nets-empty-hint").text()).toBe(EMPTY_HINT);
      expect(wrapper.get("#nets-save").attributes("disabled")).toBeDefined();
    });

    it("after saving there is no router hint", async () => {
      const { wrapper } = setup(laptopVpnState({ access: "localnet" }));
      await wrapper.get("#net-utun3").setValue(false);
      await wrapper.get("#nets-save").trigger("click");
      await settled();
      expect(wrapper.get("#nets-saved-note").text()).toBe(SAVED_NOTE);
      expect(wrapper.find("#nets-router-hint").exists()).toBe(false);
    });
  });
});

function setup(state, context = "settings") {
  return (mounted = mountWithNode(NetworksBlock, state, { props: { context } }));
}

const rowOf = (wrapper, name) => wrapper.findAll("label.net-row").find((r) => r.find("#net-" + name).exists());
const rowTitles = (wrapper) => wrapper.findAll("label.net-row .net-name").map((n) => n.text());
