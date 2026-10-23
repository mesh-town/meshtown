/* `memberErrorText`: the readable text of a failed member action. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { ApiError } from "../core/api.ts";
import { memberErrorText } from "./members-api.ts";

const ACTION = "update the member list";

describe("memberErrorText", () => {
  describe("the node sent a message", () => {
    it.each([
      [409, "self_target", "You can't remove your own computer"],
      [409, "not_member", "That computer isn't in the member list"],
      [409, "left", "This computer has left the network"],
      [409, "busy", "The node is starting"],
      [422, "invalid", "The member id is not valid"],
      [500, "internal", "Couldn't write the member list"],
    ])("%i %s: Couldn't …: the message with a lowercase first letter", (status, code, message) => {
      const err = new ApiError(status, code, message, { error: code, message });
      const lowered = message[0].toLowerCase() + message.slice(1);
      assert.equal(memberErrorText(err), "Couldn't " + ACTION + ": " + lowered);
    });

    it("the program name at the start keeps its capital letter", () => {
      const err = new ApiError(409, "busy", "busy", { error: "busy", message: "Mesh Town is stopping" });
      assert.equal(memberErrorText(err), "Couldn't " + ACTION + ": Mesh Town is stopping");
    });
  });

  describe("no message, only a code", () => {
    it.each([
      [409, "self_target"],
      [409, "not_member"],
      [409, "left"],
      [409, "busy"],
      [422, "invalid"],
      [500, "internal"],
    ])("%i %s: a human reason, never the raw code", (status, code) => {
      const text = memberErrorText(new ApiError(status, code));
      assert.ok(text.startsWith("Couldn't " + ACTION + ": "), text);
      assert.ok(!text.endsWith(": " + code), "the raw code is the whole reason: " + text);
      assert.ok(!text.includes("_"), "a machine code leaked: " + text);
      assert.ok(text.length > ("Couldn't " + ACTION + ": ").length + 5, "the reason is empty: " + text);
    });

    it("different codes give different reasons", () => {
      const codes = ["self_target", "not_member", "left", "busy", "invalid", "internal"];
      const texts = new Set(codes.map((code) => memberErrorText(new ApiError(409, code))));
      assert.equal(texts.size, codes.length);
    });
  });

  describe("the page could not reach the node", () => {
    it("network: the message of the api layer", () => {
      const err = new ApiError(0, "network", "Mesh Town is not responding");
      assert.equal(memberErrorText(err), "Couldn't " + ACTION + ": Mesh Town is not responding");
    });

    it("timeout: the message of the api layer", () => {
      const err = new ApiError(0, "timeout", "Mesh Town didn't respond in time");
      assert.equal(memberErrorText(err), "Couldn't " + ACTION + ": Mesh Town didn't respond in time");
    });
  });

  describe("not an api error", () => {
    it("a plain Error: its message", () => {
      assert.equal(memberErrorText(new Error("Boom")), "Couldn't " + ACTION + ": boom");
    });

    it.each([[null], [undefined], [{}]])("%j: the generic phrase with a hint", (err) => {
      assert.equal(memberErrorText(err), "Couldn't " + ACTION + " — try again");
    });
  });
});
