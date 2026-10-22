/* `conn.ts::absence`: "why the page gets no snapshots" — one flag for the header and "Home".
 * The order as in the header status line: waiting for a token > the node is silent (the "not responding" strip) > the node is stopped.
 * The output reads as a phrase: `absence > node_is_silent > silent_after_two_failures`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import * as connModule from "./conn.ts";
import { createConn, onBadOrigin, onBadToken, onFailure, onStopping, onSuccess } from "./conn.ts";

describe("absence", () => {
  describe("connection is fine", () => {
    it("fresh connection — null", () => {
      assert.equal(absenceOf(createConn()), null);
    });

    it("one poll failure — null: there is no \"not responding\" strip yet", () => {
      const conn = createConn();
      onFailure(conn);
      assert.equal(absenceOf(conn), null);
    });

    it("the page is opened from the wrong address (bad_origin) — null: this is not an absence of node snapshots", () => {
      const conn = createConn();
      onBadOrigin(conn, null);
      assert.equal(absenceOf(conn), null);
    });
  });

  describe("node is silent", () => {
    it("two poll failures in a row — \"silent\"", () => {
      const conn = createConn();
      onFailure(conn);
      onFailure(conn);
      assert.equal(absenceOf(conn), "silent");
    });

    it("a successful poll after two failures — null again", () => {
      const conn = createConn();
      onFailure(conn);
      onFailure(conn);
      onSuccess(conn);
      assert.equal(absenceOf(conn), null);
    });

    it("the \"not responding\" strip and the stop at once — \"silent\" comes before \"stopped\"", () => {
      const conn = createConn();
      conn.strip = { kind: "disconnected", text: "Mesh Town is not responding" };
      conn.stopped = true;
      assert.equal(absenceOf(conn), "silent");
    });
  });

  describe("node is stopped", () => {
    it("a 503 response (onStopping) — \"stopped\"", () => {
      const conn = createConn();
      onStopping(conn);
      assert.equal(absenceOf(conn), "stopped");
    });

    it("a successful poll after the stop — null again", () => {
      const conn = createConn();
      onStopping(conn);
      onSuccess(conn);
      assert.equal(absenceOf(conn), null);
    });
  });

  describe("waiting for a new link (awaitingToken)", () => {
    it("a 401 response (onBadToken) — \"token\"", () => {
      const conn = createConn();
      onBadToken(conn);
      assert.equal(absenceOf(conn), "token");
    });

    it("\"token\" beats \"silent\": the \"not responding\" strip was there, the token has expired", () => {
      const conn = createConn();
      conn.strip = { kind: "disconnected", text: "Mesh Town is not responding" };
      conn.awaitingToken = true;
      assert.equal(absenceOf(conn), "token");
    });

    it("\"token\" beats \"stopped\"", () => {
      const conn = createConn();
      onStopping(conn);
      conn.awaitingToken = true;
      assert.equal(absenceOf(conn), "token");
    });
  });
});

// Via the namespace: no export — the test assertion fails, not the file parse.
function absenceOf(conn) {
  assert.equal(typeof connModule.absence, "function", "shell/conn.ts has no absence(conn) export");
  return connModule.absence(conn);
}
