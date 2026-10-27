/* `termParts`: splitting setup text into plain pieces and the two explained terms (PAC, SOCKS5). */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { TERM_TIPS, termParts } from "./term-hints.ts";

const joined = (parts) => parts.map((p) => p.text).join("");

describe("termParts", () => {
  describe("a term in the middle of a sentence", () => {
    it("gives three pieces with the tip on the middle one", () => {
      const parts = termParts("Copy this address (PAC): then paste it");
      assert.deepEqual(parts.map((p) => p.text), ["Copy this address (", "PAC", "): then paste it"]);
      assert.equal(parts[0].tip, undefined);
      assert.equal(parts[1].tip, TERM_TIPS.PAC);
      assert.equal(parts[2].tip, undefined);
    });

    it("joins back into the original string", () => {
      const text = "Open the PAC address in the browser settings.";
      assert.equal(joined(termParts(text)), text);
    });
  });

  describe("a word that only contains a term", () => {
    it("PACs is not a term", () => {
      assert.deepEqual(termParts("PACs are small"), [{ text: "PACs are small" }]);
    });

    it("lower-case pac is not a term", () => {
      assert.deepEqual(termParts("the pac file"), [{ text: "the pac file" }]);
    });

    it("a longer word ending in PAC is not a term", () => {
      assert.deepEqual(termParts("IMPAC"), [{ text: "IMPAC" }]);
    });
  });

  describe("SOCKS5", () => {
    it("carries its own tip", () => {
      const parts = termParts("proxy type SOCKS5.");
      const term = parts.find((p) => p.text === "SOCKS5");
      assert.ok(term, "SOCKS5 is a separate piece");
      assert.equal(term.tip, TERM_TIPS.SOCKS5);
      assert.notEqual(TERM_TIPS.SOCKS5, TERM_TIPS.PAC);
    });
  });

  describe("two terms in one string", () => {
    it("marks both and keeps the text between them", () => {
      const text = "PAC or SOCKS5";
      const parts = termParts(text);
      assert.deepEqual(parts.map((p) => p.text), ["PAC", " or ", "SOCKS5"]);
      assert.deepEqual(parts.map((p) => p.tip), [TERM_TIPS.PAC, undefined, TERM_TIPS.SOCKS5]);
      assert.equal(joined(parts), text);
    });
  });

  describe("a string without terms", () => {
    it("is one plain piece", () => {
      assert.deepEqual(termParts("Choose an exit"), [{ text: "Choose an exit" }]);
    });

    it("an empty string gives no pieces with text", () => {
      assert.equal(joined(termParts("")), "");
    });
  });
});

describe("TERM_TIPS", () => {
  it("has a non-empty explanation for each term", () => {
    for (const term of ["PAC", "SOCKS5"]) {
      assert.ok(TERM_TIPS[term] && TERM_TIPS[term].length > 10, term + " has a tip");
    }
  });
});
