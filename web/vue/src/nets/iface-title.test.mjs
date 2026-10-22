/* `ifaceTitle`: the network name for a human per the naming table. Rules are checked in order:
 * the first matching one fires, so there are "this one beats that one" pairs. */

import { describe, expect, it } from "vitest";
import { ifaceTitle } from "./iface-title.ts";

const net = (name, addr, extra = {}) => ({ name, addrs: addr ? [addr] : [], default_route: false, present: true, open: true, by: "auto", ...extra });
const ROUTER_NETS = [net("br-lan", "192.168.88.1"), net("eth0", "192.168.1.100", { default_route: true })];
const ROUTER_GUEST_NETS = [net("br-lan", "192.168.88.1"), net("br-guest", "192.168.9.1"), net("eth0", "192.168.1.100", { default_route: true })];
const LAPTOP_NETS = [net("en0", "192.168.1.23", { default_route: true })];
const PROVIDER_HINT = "The router reaches the internet through this network";
const HOME_HINT = "Phones and computers connected to the router";
const VM = "Network of virtual machines and containers";

describe("ifaceTitle", () => {
  describe("naming table: name, explanation, name · address", () => {
    it.each([
      ["router, default route", net("eth0", "192.168.1.100", { default_route: true }), true, ROUTER_NETS, "ISP network", PROVIDER_HINT, "eth0 · 192.168.1.100"],
      ["router, br-lan", net("br-lan", "192.168.88.1"), true, ROUTER_NETS, "Home network", HOME_HINT, "br-lan · 192.168.88.1"],
      ["router, lan", net("lan", "192.168.1.1"), true, [net("lan", "192.168.1.1"), net("wan", "10.0.0.2", { default_route: true })], "Home network", HOME_HINT, "lan · 192.168.1.1"],
      ["router, the only other network with a foreign name", net("br0", "192.168.50.1"), true, [net("br0", "192.168.50.1"), net("eth0", "192.168.1.100", { default_route: true })], "Home network", HOME_HINT, "br0 · 192.168.50.1"],
      ["router, name with guest", net("br-guest", "192.168.9.1"), true, ROUTER_GUEST_NETS, "Guest network", "", "br-guest · 192.168.9.1"],
      ["laptop, name with guest", net("wlan-guest", "192.168.9.5"), false, [...LAPTOP_NETS, net("wlan-guest", "192.168.9.5")], "Guest network", "", "wlan-guest · 192.168.9.5"],
      ["tunnel by kind", net("utun3", "10.8.0.2", { kind: "tunnel" }), false, LAPTOP_NETS, "VPN connection", "", "utun3 · 10.8.0.2"],
      ["tunnel by kind, name not from the list", net("ppp0", "10.9.0.2", { kind: "tunnel" }), false, LAPTOP_NETS, "VPN connection", "", "ppp0 · 10.9.0.2"],
      ["docker", net("docker0", "172.17.0.1"), false, LAPTOP_NETS, VM, "", "docker0 · 172.17.0.1"],
      ["virtual by kind, name not from the list", net("foo0", "172.30.0.1", { kind: "virtual" }), false, LAPTOP_NETS, VM, "", "foo0 · 172.30.0.1"],
      ["laptop, default route without a known kind", net("eth0", "192.168.1.23", { default_route: true }), false, [net("eth0", "192.168.1.23", { default_route: true })], "Home network", "", "eth0 · 192.168.1.23"],
      ["everything else", net("en7", "192.168.77.2"), false, [...LAPTOP_NETS, net("en7", "192.168.77.2")], "Other network \"en7\"", "", "en7 · 192.168.77.2"],
    ])("%s", (_case, entry, router, all, title, hint, code) => {
      expect(ifaceTitle(entry, router, all)).toEqual({ title, hint, code });
    });
  });

  describe("tunnels by name", () => {
    it.each(["utun0", "tun1", "wg0", "tailscale0"])("%s — VPN connection", (name) => {
      expect(ifaceTitle(net(name, "10.8.0.2"), false, LAPTOP_NETS).title).toBe("VPN connection");
    });
  });

  describe("virtual networks by name", () => {
    it.each(["docker0", "veth12ab", "vmnet8", "vboxnet0", "bridge100"])("%s — network of virtual machines and containers", (name) => {
      expect(ifaceTitle(net(name, "172.17.0.1"), false, LAPTOP_NETS).title).toBe(VM);
    });

    it("br-* on a laptop is virtual, while on a router br-lan stays home", () => {
      expect(ifaceTitle(net("br-1a2b", "172.18.0.1"), false, LAPTOP_NETS).title).toBe(VM);
      expect(ifaceTitle(net("br-lan", "192.168.88.1"), true, ROUTER_NETS).title).toBe("Home network");
    });
  });

  describe("rule order: the first matching one wins", () => {
    it("router, default route: \"ISP network\" before the guest name", () => {
      const provider = net("eth0-guest", "192.168.1.100", { default_route: true });
      expect(ifaceTitle(provider, true, [net("br-lan", "192.168.88.1"), provider]).title).toBe("ISP network");
    });

    it("br-guest on a laptop is guest, not \"virtual machines\" (guest before br-*)", () => {
      expect(ifaceTitle(net("br-guest", "192.168.9.1"), false, LAPTOP_NETS).title).toBe("Guest network");
    });

    it("a tunnel with a default route on a laptop is VPN, not \"Home network\"", () => {
      const tunnel = net("utun3", "10.8.0.2", { kind: "tunnel", default_route: true });
      expect(ifaceTitle(tunnel, false, [tunnel]).title).toBe("VPN connection");
    });

    it("docker on a laptop with a default route is a virtual network, not \"Home network\"", () => {
      const docker = net("docker0", "172.17.0.1", { default_route: true });
      expect(ifaceTitle(docker, false, [docker]).title).toBe(VM);
    });

    it("on a router with three networks the second non-ISP one does not become \"Home\": only br-lan/lan", () => {
      expect(ifaceTitle(net("br-office", "192.168.40.1"), true, [net("br-lan", "192.168.88.1"), net("br-office", "192.168.40.1"), net("eth0", "192.168.1.100", { default_route: true })]).title)
        .toBe("Other network \"br-office\"");
    });
  });

  describe("name · address caption", () => {
    it("a network without an address (name from the list, no interface now) — the name only", () => {
      const absent = net("br-guest", null, { present: false, open: false });
      expect(ifaceTitle(absent, true, [...ROUTER_NETS, absent]).code).toBe("br-guest");
    });
  });
});
