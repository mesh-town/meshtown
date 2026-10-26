/* `statusLine(state, conn, members, route, refused)`: the fifth parameter `refused` is `{ names: string[] } | null` (the result of
 * `admissionRefused`, already held for the delay). With no `route` override a refusal makes the line red: "Sites won't open · <net> · <others tail>". */

import { describe, expect, it } from "vitest";
import { createConn } from "./conn.ts";
import { statusLine } from "./status-line.ts";
import { membersReply } from "../testing/members-fixtures.mjs";

const REFUSED = { names: ["peer-one"] };
const VIA = { level: "green", text: "Sites open via exit-e (work)" };
const WONT = { level: "red", text: "Sites won't open" };

describe("statusLine with a held admission refusal", () => {
  describe("no route override", () => {
    it("red \"Sites won't open\" instead of \"Sites open directly\", with the network and the others tail", () => {
      const got = statusLine(joined({ peers: [SELF, OFFLINE], transport: dht(true) }), createConn(), null, null, REFUSED);
      expect(got).toEqual({ level: "red", text: "Sites won't open · family · the other member is offline" });
    });

    it("the tail is the usual one: all other members offline", () => {
      const got = statusLine(joined({ peers: [SELF, OFFLINE, OFFLINE], transport: dht(true) }), createConn(), null, null, REFUSED);
      expect(got.text).toBe("Sites won't open · family · all other members are offline");
    });

    // Note: the rule only says "instead of Sites open directly"; this reads it as "the refusal also beats an exit line of the leading network".
    it("the refusal also wins over an exit route of the leading network", () => {
      const route = { state: "exit", exits: [{ id: "bbbb2222", name: "peer-one", active: true }] };
      const got = statusLine(joined({ route, peers: [SELF, OFFLINE], transport: dht(true) }), createConn(), null, null, REFUSED);
      expect(got.level).toBe("red");
      expect(got.text).toMatch(/^Sites won't open · family · /);
    });

    it("refused = null: the line is the usual one", () => {
      const got = statusLine(joined({ peers: [SELF, OFFLINE], transport: dht(true) }), createConn(), null, null, null);
      expect(got).toEqual({ level: "green", text: "Sites open directly · family · the other member is offline" });
    });

    it("refused omitted: the same as before the parameter existed", () => {
      const got = statusLine(joined({ peers: [SELF, OFFLINE], transport: dht(true) }), createConn(), null, null);
      expect(got).toEqual({ level: "green", text: "Sites open directly · family · the other member is offline" });
    });
  });

  describe("a route override (several networks)", () => {
    it("green \"via\": the route decides, the refusal does not repaint the line", () => {
      const got = statusLine(joined({ peers: [SELF, OFFLINE], transport: dht(true) }), createConn(), null, VIA, REFUSED);
      expect(got).toEqual({ level: "green", text: "Sites open via exit-e (work) · family · the other member is offline" });
    });

    it("red \"won't open\" of the route: stays as the route says", () => {
      const got = statusLine(joined({ peers: [SELF, OFFLINE], transport: dht(true) }), createConn(), null, WONT, REFUSED);
      expect(got).toEqual({ level: "red", text: "Sites won't open · family · the other member is offline" });
    });
  });

  describe("older branches win over the refusal", () => {
    it("no network: grey \"not joined\"", () => {
      const got = statusLine(joined({ network: null }), createConn(), null, null, REFUSED);
      expect(got).toEqual({ level: "grey", text: "You haven't joined a Mesh Town network yet" });
    });

    it("excluded: the red ban line", () => {
      const got = statusLine(joined({ members: { me_excluded: true } }), createConn(), null, null, REFUSED);
      expect(got.level).toBe("red");
      expect(got.text).toMatch(/^Removed from the network "family"/);
    });

    it("starting and stopping: grey wait lines", () => {
      expect(statusLine(joined({ node: { status: "starting" } }), createConn(), null, null, REFUSED)).toEqual({ level: "grey", text: "Mesh Town is starting…" });
      expect(statusLine(joined({ node: { status: "stopping" } }), createConn(), null, null, REFUSED)).toEqual({ level: "grey", text: "Mesh Town is stopping…" });
    });

    it("the node is stopped or silent: the grey absence line", () => {
      const stopped = createConn();
      stopped.stopped = true;
      expect(statusLine(joined(), stopped, null, null, REFUSED)).toEqual({ level: "grey", text: "Mesh Town is stopped" });
      expect(statusLine(null, createConn(), null, null, REFUSED)).toEqual({ level: "grey", text: "Mesh Town is not responding" });
    });

    it("awaiting a page link: the grey token line", () => {
      const conn = createConn();
      conn.awaitingToken = true;
      expect(statusLine(joined(), conn, null, null, REFUSED)).toEqual({ level: "grey", text: "A new page link is needed" });
    });
  });
});

function joined(over = {}) {
  return { node: { status: "running" }, network: { label: "family" }, route: { state: "direct" }, peers: [], ...over };
}

const SELF = { self: true, online: true };
const OFFLINE = { self: false, online: false };
const dht = (joined) => ({ dht: { joined, nodes: 0 } });
