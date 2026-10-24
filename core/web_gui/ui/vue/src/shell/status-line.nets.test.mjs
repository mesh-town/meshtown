/* The header line with several networks: how sites open comes from the network that carries the proxy. */

import { describe, expect, it } from "vitest";
import { createConn } from "./conn.ts";
import { statusLine } from "./status-line.ts";
import { routeLine } from "../home/route-line.ts";
import { netsStore } from "../testing/nets-store.mjs";

const X = "D".repeat(64);
const Y = "E".repeat(64);
const net = (id, label, offers, exits, proxy) => ({ id, label, offers, exits, proxy });
const line = (store) => statusLine(store.state, createConn(), store.members, routeLine(store));
const TAIL = " · home · 1 member online";

describe("statusLine with several networks", () => {
  it("exit in the leading network: via the member", () => {
    const l = line(netsStore([net("main", "home", [X], [X], true), net("k7q2xa", "work", [Y], [], false)]));
    expect(l.text).toBe("Sites open via exit-d" + TAIL);
    expect(l.level).toBe("green");
  });

  it("exit in another network: via the member and the network", () => {
    const l = line(netsStore([net("main", "home", [X], [], false), net("k7q2xa", "work", [Y], [Y], true)]));
    expect(l.text).toBe("Sites open via exit-e (work)" + TAIL);
    expect(l.level).toBe("green");
  });

  it("directly", () => {
    const l = line(netsStore([net("main", "home", [X], [], true), net("k7q2xa", "work", [Y], [], false)]));
    expect(l.text).toBe("Sites open directly" + TAIL);
    expect(l.level).toBe("green");
  });

  it("no network carries the proxy: won't open, red", () => {
    const l = line(netsStore([net("main", "home", [X], [], false), net("k7q2xa", "work", [Y], [], false)]));
    expect(l.text).toBe("Sites won't open" + TAIL);
    expect(l.level).toBe("red");
  });

  it("the chosen member stopped offering an exit: won't open, red", () => {
    const l = line(netsStore([net("main", "home", [X], [], false), net("k7q2xa", "work", [{ full: Y, exit: false }], [Y], true)]));
    expect(l.text).toBe("Sites won't open" + TAIL);
    expect(l.level).toBe("red");
  });

  it("one network: no override", () => {
    const store = netsStore([net("main", "home", [X], [X], true)]);
    expect(routeLine(store)).toBeNull();
  });
});
