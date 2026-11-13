/* The ring of one network tile: only its own members, the exit arc only in the proxy network. */

import { describe, expect, it } from "vitest";
import { netRing } from "./net-ring.ts";

const peer = (id, extra = {}) => ({ id, id_full: `${id}_full`, name: null, self: false, online: true, ...extra });
const route = { state: "exit", exits: [{ id: "ex", id_full: "ex_full", name: null, online: true, active: true }] };
const stateOf = (peers, withRoute = true) => ({ peers: [peer("me", { self: true }), ...peers], ...(withRoute ? { route } : {}), exit: { enabled: false, domains: 0, clients: [] } });
const brief = (id, proxy) => ({ id, id_hex: id, label: id, proxy, peers_total: 1, peers_online: 1, members_admitting: 0 });

function store({ a, b }) {
  const slots = { main: { state: a }, other: { state: b } };
  return { nets: [brief("main", true), brief("other", false)], slots };
}

describe("netRing", () => {
  it("ring of a net lists only its members", () => {
    const s = store({ a: stateOf([peer("aa")], false), b: stateOf([peer("bb"), peer("cc")], false) });
    expect(netRing(s, "main").members.map((m) => m.label)).toEqual(["aa"]);
    expect(netRing(s, "other").members.map((m) => m.label)).toEqual(["bb", "cc"]);
  });

  it("exit arc only in the proxy net", () => {
    const s = store({ a: stateOf([peer("ex")]), b: stateOf([peer("ex")]) });
    expect(netRing(s, "main").members.map((m) => m.role)).toContain("via");
    expect(netRing(s, "other").members.map((m) => m.role)).not.toContain("via");
  });

  it("no state → empty ring, no-network center", () => {
    const s = store({ a: stateOf([]), b: null });
    const ring = netRing(s, "other");
    expect(ring.members).toEqual([]);
    expect(ring.center).toEqual({ main: "no network", sub: "" });
  });
});
