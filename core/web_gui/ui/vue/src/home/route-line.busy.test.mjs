/* The header head with several networks when the exit of the proxy network is overloaded. */

import { describe, expect, it } from "vitest";
import { routeLine } from "./route-line.ts";
import { statePanel } from "./home-view.ts";
import { netsStore } from "../testing/nets-store.mjs";
import { baseState } from "../testing/section.mjs";

const X = "D".repeat(64);
const Y = "E".repeat(64);
const net = (id, label, offers, exits, proxy) => ({ id, label, offers, exits, proxy });

function busyStore(over) {
  const store = netsStore([net("main", "home", [X], [], false), net("k7q2xa", "work", [Y], [Y], true)]);
  const route = store.slots.k7q2xa.state.route;
  route.exits = route.exits.map((e) => ({ ...e, ...over }));
  return store;
}

describe("routeLine with an overloaded exit", () => {
  it("multi-network head is orange", () => {
    const head = routeLine(busyStore({ busy: true, overloaded: "limit" }));
    expect(head.level).toBe("orange");
    expect(head.text).toBe("Sites open via exit-e (work) — overloaded");
    const panel = statePanel(baseState(), null, null, { multi: true, netOf: () => "work", head });
    expect(panel.level).toBe("orange");
    expect(panel.main).toBe("Sites open via exit-e (work), but it is overloaded");
    expect(panel.actions).toEqual(["pick-other"]);
    expect(routeLine(busyStore({ busy: false })).level).toBe("green");
  });
});
