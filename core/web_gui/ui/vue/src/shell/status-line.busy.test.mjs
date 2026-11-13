/* The header line for a chosen exit that is overloaded. */

import { describe, expect, it } from "vitest";
import { createConn } from "./conn.ts";
import { statusLine } from "./status-line.ts";
import { baseState, PEER_FULL } from "../testing/section.mjs";

describe("statusLine with an overloaded exit", () => {
  it("header line says overloaded", () => {
    const exit = { id: "bbbb2222", id_full: PEER_FULL, name: "peer-one", country: "DE", exit: true, online: true, active: true, busy: true, overloaded: "share" };
    const state = baseState({ network: { id_hex: "net-aaaa", masked: "m", label: "home" }, route: { state: "exit", exits: [exit] } });
    const l = statusLine(state, createConn());
    expect(l.level).toBe("orange");
    expect(l.text.startsWith("Sites open via peer-one — overloaded")).toBe(true);
    const calm = statusLine(baseState({ network: state.network, route: { state: "exit", exits: [{ ...exit, busy: false, overloaded: null }] } }), createConn());
    expect(calm.level).toBe("green");
  });
});
