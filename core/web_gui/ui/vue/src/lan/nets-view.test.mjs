/* Pure functions of the networks block over a node snapshot and the config (`nets-view.ts`). Snapshots are in `testing/nets-states.mjs`. */

import { describe, expect, it } from "vitest";
import { laptopState, laptopVpnState, routerGuestState, routerState } from "../testing/nets-states.mjs";
import {
  AUTO_PAYLOAD, blockWhere, draftNames, isDirty, isRouter, lanMode, lanView, networksOf, openBySaved, routerPacUrl, sameNetsLine, savePayload, summaryView, viaIface,
} from "./nets-view.ts";

const cfg = (socks, web, ifaces = []) => ({ socks: { access: socks, ifaces }, web: { access: web } });

describe("nets-view", () => {
  describe("reading the snapshot", () => {
    it("router: isRouter, viaIface and the networks in snapshot order", () => {
      const state = routerState();
      expect(isRouter(state)).toBe(true);
      expect(viaIface(state)).toBe("br-lan");
      expect(networksOf(state).map((n) => n.name)).toEqual(["br-lan", "eth0"]);
    });

    it("an older node (no node.networks): not a router, via_iface null, no networks", () => {
      const old = { node: {}, socks: {} };
      expect(isRouter(old)).toBe(false);
      expect(viaIface(old)).toBe(null);
      expect(networksOf(old)).toEqual([]);
    });
  });

  describe("openBySaved: whether a network is open by the saved list", () => {
    const [home, provider] = routerState().node.networks;

    it("list not set, router: only the default-route network is closed", () => {
      expect(openBySaved(home, [], true)).toBe(true);
      expect(openBySaved(provider, [], true)).toBe(false);
      expect(openBySaved(provider, undefined, true)).toBe(false);
    });

    it("list not set, not a router: everything is open", () => {
      expect(openBySaved(provider, [], false)).toBe(true);
    });

    it("list set: names from the list are open, with an explicit list the ISP network can be opened", () => {
      expect(openBySaved(home, ["br-lan"], true)).toBe(true);
      expect(openBySaved(provider, ["br-lan"], true)).toBe(false);
      expect(openBySaved(provider, ["br-lan", "eth0"], true)).toBe(true);
    });

    it('["*"] — all networks are open', () => {
      expect(openBySaved(provider, ["*"], true)).toBe(true);
    });
  });

  describe("checkbox draft", () => {
    it("draftNames: open networks in snapshot order (automatic on a router — only br-lan)", () => {
      expect(draftNames(routerState(), [])).toEqual(["br-lan"]);
      expect(draftNames(routerGuestState(), ["br-lan"])).toEqual(["br-lan"]);
      expect(draftNames(routerGuestState(), ["eth0", "br-lan"])).toEqual(["br-lan", "eth0"]);
    });

    it("isDirty: the same set in another order is not a change; another set is", () => {
      const state = routerGuestState();
      expect(isDirty(["br-lan"], state, ["br-lan"])).toBe(false);
      expect(isDirty(["br-lan", "eth0"], state, ["br-lan", "eth0"])).toBe(false);
      expect(isDirty(["eth0", "br-lan"], state, ["br-lan", "eth0"])).toBe(false);
      expect(isDirty(["br-lan", "eth0"], state, ["br-lan"])).toBe(true);
    });
  });

  describe("blockWhere: where the networks block stands", () => {
    it("two networks and the proxy \"local network\" — in the main settings (main)", () => {
      expect(blockWhere(cfg("localnet", "localnet"), routerState())).toBe("main");
      expect(blockWhere(cfg("localnet", "localhost"), routerState())).toBe("main");
    });

    it("proxy \"this computer only\", page \"local network\" — in the \"Page\" section (page)", () => {
      expect(blockWhere(cfg("localhost", "localnet"), routerState())).toBe("page");
    });

    it("both \"this computer only\" — no block", () => {
      expect(blockWhere(cfg("localhost", "localhost"), routerState())).toBe(null);
    });

    it("one network — no block, the single network is not a block", () => {
      // Open question: one network with web.access=localnet could be read as "page"; the chosen reading is no block.
      expect(blockWhere(cfg("localnet", "localnet"), laptopState())).toBe(null);
    });

    it("two networks on a laptop (VPN) — there is a block", () => {
      expect(blockWhere(cfg("localnet", "localhost"), laptopVpnState())).toBe("main");
    });
  });

  describe("sameNetsLine: the line in the \"Page\" section when the block is in the main settings", () => {
    it("block in main and page \"local network\" — names of the networks open by the saved list", () => {
      expect(sameNetsLine(cfg("localnet", "localnet"), routerState())).toBe("From the same networks as the proxy: Home network.");
      expect(sameNetsLine(cfg("localnet", "localnet", ["br-lan", "eth0"]), routerState())).toBe("From the same networks as the proxy: Home network, ISP network.");
    });

    it("page \"this computer only\" or the block is not in main — no line", () => {
      expect(sameNetsLine(cfg("localnet", "localhost"), routerState())).toBe(null);
      expect(sameNetsLine(cfg("localhost", "localnet"), routerState())).toBe(null);
    });
  });

  describe("summaryView: the \"The proxy is open for\" summary", () => {
    it("automatic: home is open, a tail about the ISP network", () => {
      expect(summaryView(routerState(), [])).toEqual({ open: ["Home network"], tail: " The ISP network is closed automatically." });
    });

    it("list set: the \"Closed for\" tail with the closed names in snapshot order", () => {
      expect(summaryView(routerGuestState(), ["br-lan"])).toEqual({ open: ["Home network"], tail: " Closed for: Guest network, ISP network." });
    });

    it("nothing closed — no tail", () => {
      expect(summaryView(routerGuestState(), ["*"])).toEqual({ open: ["Home network", "Guest network", "ISP network"], tail: "" });
    });
  });

  describe("routerPacUrl: the auto-configuration address on a router", () => {
    it("router, proxy open for the home network — PAC at the home network address", () => {
      expect(routerPacUrl(routerState())).toBe("http://192.168.88.1:1080/proxy.pac");
    });

    it("several networks open — \"Home network\" is taken, not the first in socks.lan", () => {
      const state = routerGuestState({ ifaces: ["br-guest", "br-lan"] });
      state.socks.lan = [{ addr: "192.168.9.1:1080", iface: "br-guest" }, { addr: "192.168.88.1:1080", iface: "br-lan" }];
      expect(routerPacUrl(state)).toBe("http://192.168.88.1:1080/proxy.pac");
    });

    it("no home network among the open ones — the first open one", () => {
      const state = routerGuestState({ ifaces: ["br-guest"] });
      expect(routerPacUrl(state)).toBe("http://192.168.9.1:1080/proxy.pac");
    });

    it("the node sent the address of the request's network (pac_url not loopback) — it is taken, not \"Home network\"", () => {
      const state = routerGuestState({ ifaces: ["br-guest", "br-lan"] });
      state.socks.pac_url = "http://192.168.9.1:1080/proxy.pac";
      expect(routerPacUrl(state)).toBe("http://192.168.9.1:1080/proxy.pac");
    });

    it("pac_url points to 127.0.0.1 or localhost — the address from socks.lan, as before", () => {
      for (const pac of ["http://127.0.0.1:1080/proxy.pac", "http://localhost:1080/proxy.pac", "http://[::1]:1080/proxy.pac"]) {
        const state = routerState();
        state.socks.pac_url = pac;
        expect(routerPacUrl(state), pac).toBe("http://192.168.88.1:1080/proxy.pac");
      }
    });

    it("laptop with a non-loopback pac_url — null: the rule is for routers only", () => {
      const state = laptopState({ access: "localnet" });
      state.socks.pac_url = "http://192.168.1.23:1080/proxy.pac";
      expect(routerPacUrl(state)).toBe(null);
    });

    it("laptop and router with the proxy \"this computer only\" (socks.lan empty) — null: socks.pac_url stays", () => {
      expect(routerPacUrl(laptopState({ access: "localnet" }))).toBe(null);
      expect(routerPacUrl(routerState({ access: "localhost" }))).toBe(null);
    });
  });

  describe("lanView: the address line for other devices", () => {
    it("a node without node.networks — the old look (legacy)", () => {
      expect(lanView({ node: {}, socks: { enabled: true, access: "localnet", lan: [{ addr: "192.168.1.5:1080", iface: "eth0" }] } }).kind).toBe("legacy");
    });

    it("proxy only for this computer — no line", () => {
      expect(lanView(routerState({ access: "localhost" }))).toBe(null);
    });

    it("router, one home network open — home, no hint", () => {
      expect(lanView(routerState())).toEqual({ kind: "home", items: [{ title: "Home network", addr: "192.168.88.1:1080" }], hint: "" });
    });

    it("laptop, one network open — single", () => {
      const view = lanView(laptopState({ access: "localnet" }));
      expect(view.kind).toBe("single");
      expect(view.items).toEqual([{ title: "Home network", addr: "192.168.1.23:1080" }]);
      expect(view.hint).toBe("");
    });

    it("several networks open — multi, names by role and the hint \"Use the address of the network…\"", () => {
      const view = lanView(routerGuestState({ ifaces: ["br-lan", "br-guest"] }));
      expect(view.kind).toBe("multi");
      expect(view.items).toEqual([{ title: "Home network", addr: "192.168.88.1:1080" }, { title: "Guest network", addr: "192.168.9.1:1080" }]);
      expect(view.hint).toBe("Use the address of the network the device is connected to.");
    });

    it("the interface name is not shown in the line", () => {
      const text = JSON.stringify(lanView(routerGuestState({ ifaces: ["br-lan", "br-guest"] })));
      expect(text).not.toMatch(/br-lan|br-guest|eth0/);
    });
  });

  describe("lanMode: how other devices reach the proxy", () => {
    it("a router — router, open or not", () => {
      expect(lanMode(routerState())).toBe("router");
      expect(lanMode(routerState({ access: "localhost" }))).toBe("router");
    });

    it("not a router, open to the local network with an address — shared", () => {
      expect(lanMode(laptopState({ access: "localnet" }))).toBe("shared");
    });

    it("not a router, closed or without addresses — none", () => {
      expect(lanMode(laptopState({ access: "localhost" }))).toBe("none");
      expect(lanMode({ node: {}, socks: { enabled: true, access: "localnet", lan: [] } })).toBe("none");
    });
  });

  describe("saving", () => {
    it("savePayload: one list into both fields", () => {
      expect(savePayload(["br-lan", "eth0"])).toEqual({ "socks.ifaces": ["br-lan", "eth0"], "web.ifaces": ["br-lan", "eth0"] });
    });

    it("AUTO_PAYLOAD: \"Return to automatic choice\" — both lists empty", () => {
      expect(AUTO_PAYLOAD).toEqual({ "socks.ifaces": [], "web.ifaces": [] });
    });
  });
});
