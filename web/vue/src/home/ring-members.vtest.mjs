/* Ring members from the node snapshot: roles, the active exit, order. */

import { describe, expect, it } from "vitest";
import { ringMembers } from "./ring-members.ts";

describe("ringMembers", () => {
  describe("no data", () => {
    it("null gives an empty list", () => {
      expect(ringMembers(null)).toEqual([]);
    });

    it("a snapshot without peers gives an empty list", () => {
      expect(ringMembers({})).toEqual([]);
    });

    it("only the own node — the ring is empty", () => {
      expect(ringMembers(snap({ peers: [peer("me", { self: true })] }))).toEqual([]);
    });
  });

  describe("roles without an exit", () => {
    it("the own node does not get into the ring", () => {
      const m = ringMembers(snap({ peers: [peer("me", { self: true }), peer("aa")] }));
      expect(m.map((x) => x.label)).toEqual(["aa"]);
    });

    it("a member online — online, offline — offline", () => {
      const m = ringMembers(snap({ peers: [peer("on"), peer("off", { online: false })] }));
      expect(roles(m)).toEqual({ on: "online", off: "offline" });
    });

    it("a client from exit.clients — client, the rest online", () => {
      const m = ringMembers(snap({ peers: [peer("cl"), peer("other")], clients: ["cl_full"] }));
      expect(roles(m)).toEqual({ cl: "client", other: "online" });
    });

    it("a client that is offline — offline: connectivity decides everything", () => {
      const m = ringMembers(snap({ peers: [peer("cl", { online: false })], clients: ["cl_full"] }));
      expect(roles(m)).toEqual({ cl: "offline" });
    });
  });

  describe("the active exit among peers", () => {
    it("gets the via role", () => {
      const m = ringMembers(snap({ peers: [peer("ex"), peer("aa")], exits: [exit("ex", { active: true })] }));
      expect(roles(m)).toEqual({ ex: "via", aa: "online" });
    });

    it("with a single exit without the active mark the first is taken", () => {
      const m = ringMembers(snap({ peers: [peer("ex"), peer("aa")], exits: [exit("ex")] }));
      expect(roles(m).ex).toBe("via");
    });

    it("without the active mark the first of several is considered active", () => {
      const exits = [exit("e1"), exit("e2")];
      const m = ringMembers(snap({ peers: [peer("e1"), peer("e2")], exits }));
      expect(roles(m)).toEqual({ e1: "via", e2: "online" });
    });

    it("the one marked active beats the first in the list", () => {
      const exits = [exit("e1"), exit("e2", { active: true })];
      const m = ringMembers(snap({ peers: [peer("e1"), peer("e2")], exits }));
      expect(roles(m)).toEqual({ e1: "online", e2: "via" });
    });

    it("an exit that is also a client — via (blue), not client", () => {
      const m = ringMembers(snap({ peers: [peer("ex")], exits: [exit("ex", { active: true })], clients: ["ex_full"] }));
      expect(roles(m)).toEqual({ ex: "via" });
    });

    it("an exit that is offline — offline, grey, via is not drawn", () => {
      const m = ringMembers(snap({ peers: [peer("ex", { online: false })], exits: [exit("ex", { active: true })] }));
      expect(m).toHaveLength(1);
      expect(roles(m)).toEqual({ ex: "offline" });
    });
  });

  describe("active exit outside peers", () => {
    it("one extra via member with the exit's name is added", () => {
      const exits = [exit("zz", { active: true, name: "Exit-Z" })];
      const m = ringMembers(snap({ peers: [peer("aa")], exits }));
      expect(m).toHaveLength(2);
      expect(m.filter((x) => x.role === "via")).toEqual([{ role: "via", label: "Exit-Z", idFull: "zz_full" }]);
    });

    it("without a name the label is the exit's short id", () => {
      const m = ringMembers(snap({ peers: [], exits: [exit("zz", { active: true })] }));
      expect(m).toEqual([{ role: "via", label: "zz", idFull: "zz_full" }]);
    });
  });

  describe("route.state is not exit", () => {
    it("direct — no blue, even if exits has an active one", () => {
      const m = ringMembers(snap({ peers: [peer("ex")], route: "direct", exits: [exit("ex", { active: true })] }));
      expect(roles(m)).toEqual({ ex: "online" });
    });

    it("direct — no extra via for an exit outside peers is added", () => {
      const m = ringMembers(snap({ peers: [], route: "direct", exits: [exit("zz", { active: true })] }));
      expect(m).toEqual([]);
    });

    it("unavailable — no via either", () => {
      const m = ringMembers(snap({ peers: [peer("ex")], route: "unavailable", exits: [exit("ex", { active: true })] }));
      expect(roles(m)).toEqual({ ex: "online" });
    });
  });

  describe("label and order", () => {
    it("label is name, and without a name the short id", () => {
      const m = ringMembers(snap({ peers: [peer("ab", { name: "Laptop" }), peer("cd")] }));
      expect(m.map((x) => x.label).sort()).toEqual(["Laptop", "cd"].sort());
      expect(m.find((x) => x.label === "Laptop").idFull).toBe("ab_full");
    });

    it("roles go by rank: via, client, online, offline", () => {
      const peers = [peer("off", { online: false }), peer("on"), peer("cl"), peer("ex")];
      const m = ringMembers(snap({ peers, exits: [exit("ex", { active: true })], clients: ["cl_full"] }));
      expect(m.map((x) => x.role)).toEqual(["via", "client", "online", "offline"]);
    });

    it("within a role — alphabetical, not by character code", () => {
      const m = ringMembers(snap({ peers: [peer("c", { name: "cherry" }), peer("a", { name: "Banana" }), peer("b", { name: "apple" })] }));
      expect(m.map((x) => x.label)).toEqual(["apple", "Banana", "cherry"]);
    });

    it("within a role non-ASCII labels are compared alphabetically, not by character code", () => {
      const m = ringMembers(snap({ peers: [peer("x", { name: "Éva" }), peer("y", { name: "Zoe" }), peer("z", { name: "Eda" })] }));
      expect(m.map((x) => x.label)).toEqual(["Eda", "Éva", "Zoe"]);
    });
  });
});

/** Members by label → role. */
function roles(members) {
  return Object.fromEntries(members.map((m) => [m.label, m.role]));
}

function peer(id, extra = {}) {
  return { id, id_full: `${id}_full`, name: null, self: false, online: true, ...extra };
}

function exit(id, extra = {}) {
  return { id, id_full: `${id}_full`, name: null, online: true, active: false, ...extra };
}

/** Snapshot: peers, route (exit by default), exits, clients. */
function snap({ peers, route = "exit", exits = [], clients = [] }) {
  return { peers, route: { state: route, exits }, exit: { enabled: false, domains: 0, clients } };
}
