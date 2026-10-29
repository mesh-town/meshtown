/* The list and the state line of the "Send exit traffic through" field (`exit-iface-view.ts`).
 * `ifaceOptions(networks, current)`: pairs [value, label]; the first one is "any network" with the value "", then the networks
 * (VPN tunnels first), and a current name the computer doesn't have gets its own item marked "(not found)".
 * `ifaceStateText(info)`: the warning under the field, `null` when there is nothing to warn about. */

import { describe, expect, it } from "vitest";
import { ifaceOptions, ifaceStateText } from "./exit-iface-view.ts";

const ANY_LABEL = "Any network (system routing)";
const REFUSED = "exit connections are refused";

const net = (name, extra = {}) => ({ name, addrs: ["192.168.1.10"], default_route: false, present: true, open: false, by: "auto", ...extra });
const EN0 = net("en0", { default_route: true });
const ETH1 = net("eth1");
const WG0 = net("wg0", { kind: "tunnel" });
const UTUN3 = net("utun3", { kind: "tunnel" });

const values = (options) => options.map(([value]) => value);

describe("ifaceOptions", () => {
  describe("no networks", () => {
    it("has only the item for any network", () => {
      expect(ifaceOptions([], null)).toEqual([["", ANY_LABEL]]);
    });
  });

  describe("several networks and no choice", () => {
    it("starts with any network, labelled for the system routing", () => {
      const options = ifaceOptions([EN0, WG0], null);
      expect(options[0]).toEqual(["", ANY_LABEL]);
    });

    it("lists every network once, by its name as the value", () => {
      const options = ifaceOptions([EN0, ETH1, WG0], null);
      expect(values(options).slice(1).sort()).toEqual(["en0", "eth1", "wg0"]);
    });

    it("puts VPN tunnels before the other networks", () => {
      const options = ifaceOptions([EN0, WG0, ETH1, UTUN3], null);
      expect(values(options)).toEqual(["", "wg0", "utun3", "en0", "eth1"]);
    });

    it("adds nothing marked as not found", () => {
      const options = ifaceOptions([EN0, WG0], null);
      expect(options.filter(([, label]) => label.endsWith(" (not found)"))).toEqual([]);
    });
  });

  describe("the current choice is among the networks", () => {
    it("does not repeat it", () => {
      const options = ifaceOptions([EN0, WG0], "wg0");
      expect(values(options).filter((v) => v === "wg0")).toHaveLength(1);
    });

    it("does not mark it as not found", () => {
      const options = ifaceOptions([EN0, WG0], "wg0");
      expect(options.filter(([, label]) => label.endsWith(" (not found)"))).toEqual([]);
    });
  });

  describe("the current choice is not among the networks", () => {
    it("keeps it in the list under its own name", () => {
      const options = ifaceOptions([EN0, WG0], "gone0");
      expect(values(options).filter((v) => v === "gone0")).toHaveLength(1);
    });

    it("marks its label with \" (not found)\"", () => {
      const options = ifaceOptions([EN0, WG0], "gone0");
      const label = options.find(([value]) => value === "gone0")[1];
      expect(label).toContain("gone0");
      expect(label.endsWith(" (not found)")).toBe(true);
    });

    it("still starts with any network", () => {
      const options = ifaceOptions([EN0], "gone0");
      expect(options[0]).toEqual(["", ANY_LABEL]);
    });

    it("keeps it even when there are no networks at all", () => {
      expect(values(ifaceOptions([], "gone0"))).toEqual(["", "gone0"]);
    });
  });
});

describe("ifaceStateText", () => {
  it("up: nothing to warn about", () => {
    expect(ifaceStateText({ name: "wg0", state: "up" })).toBeNull();
  });

  it("down: names the interface and says that exit connections are refused", () => {
    expect(ifaceStateText({ name: "wg0", state: "down" })).toBe("wg0 is down — " + REFUSED);
  });

  it("missing: says the interface is not on this computer and that exit connections are refused", () => {
    expect(ifaceStateText({ name: "wg0", state: "missing" })).toBe("wg0 is not on this computer — " + REFUSED);
  });

  it("unsupported: says the system can't bind exit traffic to an interface", () => {
    expect(ifaceStateText({ name: "wg0", state: "unsupported" })).toBe("This system can't bind exit traffic to an interface — " + REFUSED);
  });

  it("null (any network): nothing to warn about", () => {
    expect(ifaceStateText(null)).toBeNull();
  });

  it("undefined (an older node without the field): nothing to warn about", () => {
    expect(ifaceStateText(undefined)).toBeNull();
  });
});
