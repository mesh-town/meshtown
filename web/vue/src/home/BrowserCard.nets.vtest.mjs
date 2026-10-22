/* The "Connect the browser" card on a router and on a laptop with a choice of networks: the PAC address and
 * the intro paragraph, the networks summary in step 2, the line of addresses for other devices. The rest is in `BrowserCard.vtest.mjs`. */

import { afterEach, describe, expect, it } from "vitest";
import { PAC_INTRO } from "../common/pac-texts.ts";
import { LAN_HINT_MULTI, PAC_INTRO_ROUTER } from "../nets/nets-texts.ts";
import { flat, mountWithNode } from "../testing/nets-mount.mjs";
import { laptopState, routerGuestState, routerState } from "../testing/nets-states.mjs";
import BrowserCard from "./BrowserCard.vue";

let mounted;
afterEach(() => mounted?.wrapper.unmount());

describe("BrowserCard > choice of networks", () => {
  describe("router, proxy open to the home network", () => {
    it("the PAC address is the home network address, the paragraph is about a phone or computer connected to the router", () => {
      const { wrapper } = setup(routerState());
      expect(wrapper.get("#pac-url-text").text()).toBe("http://192.168.88.1:1080/proxy.pac");
      expect(wrapper.get("#proxy-hint").text()).toBe(PAC_INTRO_ROUTER);
    });

    it("the networks summary is in step 2 right after the \"why the address\" explanation", () => {
      const { wrapper } = setup(routerState());
      const why = wrapper.get("#pac-why").element;
      const summary = wrapper.get("#nets-summary").element;
      expect(why.compareDocumentPosition(summary) & Node.DOCUMENT_POSITION_FOLLOWING, "summary after #pac-why").toBeTruthy();
    });

    it("order: the summary and the address line come before the \"While Exit via… is set\" note", () => {
      const { wrapper } = setup(routerState());
      const order = ["#pac-why", "#nets-summary", "#proxy-lan", "#pac-no-exit"].map((id) => wrapper.get(id).element);
      order.slice(1).forEach((el, i) => expect(order[i].compareDocumentPosition(el) & Node.DOCUMENT_POSITION_FOLLOWING, el.id).toBeTruthy());
    });

    it("one home network — \"For a phone and TV on the home network\", no interface names and no hint", () => {
      const { wrapper } = setup(routerState());
      expect(flat(wrapper.get("#proxy-lan"))).toBe("For a phone and TV on the home network: address 192.168.88.1:1080, type HTTP or SOCKS5");
      expect(wrapper.find("#proxy-lan-hint").exists()).toBe(false);
    });

    it("not the setup (no bare): there is no networks summary in the card", () => {
      const { wrapper } = setup(routerState(), {});
      expect(wrapper.find("#nets-summary").exists()).toBe(false);
    });
  });

  describe("router, several networks open", () => {
    it("names by role and addresses, the hint \"Use the address of the network the device is connected to.\"", () => {
      const { wrapper } = setup(routerGuestState({ ifaces: ["br-lan", "br-guest"] }));
      expect(flat(wrapper.get("#proxy-lan"))).toBe("For other devices: Home network — 192.168.88.1:1080 · Guest network — 192.168.9.1:1080, type HTTP or SOCKS5");
      expect(wrapper.get("#proxy-lan-hint").text()).toBe(LAN_HINT_MULTI);
    });

    it("the closed ISP network does not get into the line", () => {
      const { wrapper } = setup(routerGuestState({ ifaces: ["br-lan", "br-guest"] }));
      expect(wrapper.get("#proxy-lan").text()).not.toContain("192.168.1.100");
    });
  });

  describe("router, proxy only for this computer", () => {
    it("the PAC address is unchanged (loopback), the paragraph is unchanged, no line for other devices; instead of the summary — \"router only\"", () => {
      const { wrapper } = setup(routerState({ access: "localhost" }));
      expect(wrapper.get("#pac-url-text").text()).toBe("http://127.0.0.1:1080/proxy.pac");
      expect(wrapper.get("#proxy-hint").text()).toBe(PAC_INTRO);
      expect(wrapper.find("#proxy-lan").exists()).toBe(false);
      expect(wrapper.find("#router-only").exists()).toBe(true);
    });
  });

  describe("laptop", () => {
    it("proxy for the network: the address and paragraph are unchanged, no networks summary, the address line has one network", () => {
      const { wrapper } = setup(laptopState({ access: "localnet" }));
      expect(wrapper.get("#pac-url-text").text()).toBe("http://127.0.0.1:1080/proxy.pac");
      expect(wrapper.get("#proxy-hint").text()).toBe(PAC_INTRO);
      expect(wrapper.find("#nets-summary").exists()).toBe(false);
      // Open question: without kind "wifi" the laptop's default-route network is called "Home network".
      expect(flat(wrapper.get("#proxy-lan"))).toBe("For other devices on the network: address 192.168.1.23:1080 (Home network), type HTTP or SOCKS5");
    });

    it("old order: the \"While Exit via… is set\" note comes before the address line — the laptop setup doesn't change", () => {
      const { wrapper } = setup(laptopState({ access: "localnet" }));
      const before = wrapper.get("#pac-no-exit").element;
      expect(before.compareDocumentPosition(wrapper.get("#proxy-lan").element) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    });

    it("proxy only for this computer: no line for other devices", () => {
      expect(setup(laptopState()).wrapper.find("#proxy-lan").exists()).toBe(false);
    });
  });
});

function setup(state, props = { bare: true }) {
  return (mounted = mountWithNode(BrowserCard, state, { props }));
}
