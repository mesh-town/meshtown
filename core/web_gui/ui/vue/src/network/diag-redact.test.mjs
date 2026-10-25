import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { redactLine } from "./diag-redact.ts";

describe("redactLine", () => {
  it("node page with a token: the address and the token are cut out", () => {
    const out = redactLine("Mesh Town page: http://127.0.0.1:7070/?token=abc");
    assert.equal(out, "Mesh Town page: http://‹address›/?token=‹hidden›");
    assert.doesNotMatch(out, /abc|127\.0\.0\.1/);
  });

  it("start line: the network id and the node id (full and short) are replaced", () => {
    const line = "meshtown 1.0, network Mesh Town a1b2c3d4 e5f6a7b8, node \"n\" (ff00aa11), identity 0123456789abcdef";
    const out = redactLine(line, ["a1b2c3d4", "ff00aa11", "0123456789abcdef", ""]);
    assert.doesNotMatch(out, /a1b2c3d4|ff00aa11|0123456789abcdef/);
    assert.match(out, /‹id›/);
    assert.match(out, /e5f6a7b8/);
  });

  it("a short id is the beginning of the full one: the tail of the full one does not remain", () => {
    const full = "ab12cd34ef" + "0".repeat(54);
    const out = redactLine("node ab12cd34ef, identity " + full, ["ab12cd34ef", full]);
    assert.equal(out, "node ‹id›, identity ‹id›");
  });

  it("a peer's full id (long hex) is cut out even without a value list", () => {
    const out = redactLine("peer " + "9f".repeat(32) + " online");
    assert.equal(out, "peer ‹id› online");
  });

  it("IPv4 with and without a port", () => {
    assert.equal(redactLine("peer 203.0.113.7:4141 and 10.0.0.1"), "peer ‹address› and ‹address›");
  });

  it("IPv6: in brackets with a port, compressed, full", () => {
    assert.equal(redactLine("listening on [2001:db8::1]:5000"), "listening on ‹address›");
    assert.equal(redactLine("address fe80::1 ready"), "address ‹address› ready");
    assert.equal(redactLine("address ::1 ready"), "address ‹address› ready");
    assert.equal(redactLine("a 2001:db8:0:0:0:0:0:1 b"), "a ‹address› b");
  });

  it("plain text and Rust paths are left alone", () => {
    assert.equal(redactLine("meshtown::node started in 12.5s v1.2.3"), "meshtown::node started in 12.5s v1.2.3");
  });
});
