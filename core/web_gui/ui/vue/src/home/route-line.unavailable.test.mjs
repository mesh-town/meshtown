/* The header line when the network that carries the proxy reports its route as unavailable. */

import { describe, expect, it } from "vitest";
import { routeLine } from "./route-line.ts";
import { netsStore } from "../testing/nets-store.mjs";

const X = "D".repeat(64);
const Y = "E".repeat(64);
const net = (id, label, offers, exits, proxy) => ({ id, label, offers, exits, proxy });

function unavailableIn(store, slot, mutate = () => {}) {
  const state = store.slots[slot].state;
  state.route = { ...state.route, state: "unavailable" };
  mutate(state);
  return store;
}

const WONT = { level: "red", text: "Sites won't open" };

describe("routeLine with an unavailable route", () => {
  it("proxy in a non-leading slot, exit offline: won't open, red", () => {
    const store = netsStore([net("main", "home", [X], [], false), net("k7q2xa", "work", [Y], [Y], true)]);
    unavailableIn(store, "k7q2xa", (s) => s.peers.forEach((p) => { if (p.id_full === Y) p.online = false; }));
    expect(routeLine(store)).toEqual(WONT);
  });

  it("proxy in a non-leading slot, exit missing from the peers: won't open, red", () => {
    const store = netsStore([net("main", "home", [X], [], false), net("k7q2xa", "work", [Y], [Y], true)]);
    unavailableIn(store, "k7q2xa", (s) => { s.peers = s.peers.filter((p) => p.id_full !== Y); });
    expect(routeLine(store)).toEqual(WONT);
  });

  it("unavailable in the leading slot: the leading panel speaks, no override", () => {
    const store = netsStore([net("main", "home", [X], [X], true), net("k7q2xa", "work", [Y], [], false)]);
    unavailableIn(store, "main");
    expect(routeLine(store)).toBeNull();
  });

  it("exit in a non-leading slot with a working route: via the member and the network", () => {
    const store = netsStore([net("main", "home", [X], [], false), net("k7q2xa", "work", [Y], [Y], true)]);
    expect(routeLine(store)).toEqual({ level: "green", text: "Sites open via exit-e (work)" });
  });

  it("direct in a non-leading slot: no override", () => {
    const store = netsStore([net("main", "home", [X], [], false), net("k7q2xa", "work", [Y], [], true)]);
    expect(routeLine(store)).toBeNull();
  });
});
