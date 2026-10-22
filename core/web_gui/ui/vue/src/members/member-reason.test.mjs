/* `member-reason.ts`: the optional reason a member gives when proposing a removal (shown to the other members).
 *
 * Contract fixed by these tests:
 *  - `REASON_MAX` = 80, the longest reason in Unicode code points (not UTF-16 units).
 *  - `cleanReason(text: string): string`: every control character becomes a space, the ends are trimmed, and the result is cut to
 *    `REASON_MAX` code points (a surrogate pair is never split). Blank input gives "". Nothing else is changed: no escaping,
 *    no case change, inner spaces and non-ASCII letters stay as they are. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { REASON_MAX, cleanReason } from "./member-reason.ts";

const EMOJI = "\u{1F4BB}"; // one code point, two UTF-16 units
const codePoints = (text) => Array.from(text).length;
const hasLoneSurrogate = (text) => /[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/.test(text);

describe("REASON_MAX", () => {
  it("is 80", () => {
    assert.equal(REASON_MAX, 80);
  });
});

describe("cleanReason", () => {
  describe("an ordinary sentence", () => {
    it("stays as it is", () => {
      assert.equal(cleanReason("lost laptop"), "lost laptop");
    });

    it("keeps the inner spaces, the case, punctuation and non-ASCII letters", () => {
      assert.equal(cleanReason("Café  stolen, 100% sure!"), "Café  stolen, 100% sure!");
    });

    it("keeps markup characters: showing them safely is the job of the page", () => {
      assert.equal(cleanReason('<b>bold</b> & "quoted"'), '<b>bold</b> & "quoted"');
    });
  });

  describe("spaces around the text", () => {
    it("are trimmed", () => {
      assert.equal(cleanReason("   lost laptop \t"), "lost laptop");
    });

    it.each([[""], ["   "], ["\n\t \r"]])("blank input %j gives an empty string", (text) => {
      assert.equal(cleanReason(text), "");
    });
  });

  describe("control characters", () => {
    it.each([
      ["a newline", "lost\nlaptop"],
      ["a tab", "lost\tlaptop"],
      ["a carriage return", "lost\rlaptop"],
      ["a NUL", "lost\u0000laptop"],
      ["a bell", "lost\u0007laptop"],
      ["an escape", "lost\u001blaptop"],
    ])("%s inside the text becomes a space", (_name, text) => {
      assert.equal(cleanReason(text), "lost laptop");
    });

    it("at the ends they are trimmed away together with the spaces", () => {
      assert.equal(cleanReason("\n\u0000 lost laptop \r\n"), "lost laptop");
    });

    it("a text of control characters only gives an empty string", () => {
      assert.equal(cleanReason("\u0000\u0001\u0002\n"), "");
    });
  });

  describe("the length limit", () => {
    it("a reason of exactly 80 characters is kept whole", () => {
      const text = "a".repeat(REASON_MAX);
      assert.equal(cleanReason(text), text);
    });

    it("a reason of 81 characters is cut to 80", () => {
      assert.equal(cleanReason("a".repeat(REASON_MAX) + "b"), "a".repeat(REASON_MAX));
    });

    it("a very long reason is cut to its first 80 characters", () => {
      const text = "0123456789".repeat(30);
      assert.equal(cleanReason(text), text.slice(0, REASON_MAX));
    });

    it("the spaces trimmed from the start do not count towards the limit", () => {
      assert.equal(cleanReason("   " + "a".repeat(REASON_MAX)), "a".repeat(REASON_MAX));
    });

    it("counts code points, not UTF-16 units: 80 emoji stay whole", () => {
      const text = EMOJI.repeat(REASON_MAX);
      assert.equal(cleanReason(text), text);
    });

    it("81 emoji are cut to 80, whole characters", () => {
      const cut = cleanReason(EMOJI.repeat(REASON_MAX + 1));
      assert.equal(cut, EMOJI.repeat(REASON_MAX));
      assert.equal(hasLoneSurrogate(cut), false, "a surrogate pair was split");
    });

    it("a pair on the border is kept whole when it is the 80th character", () => {
      const cut = cleanReason("a".repeat(REASON_MAX - 1) + EMOJI + "b");
      assert.equal(cut, "a".repeat(REASON_MAX - 1) + EMOJI);
      assert.equal(codePoints(cut), REASON_MAX);
    });

    it("a pair that would be the 81st character is dropped whole", () => {
      const cut = cleanReason("a".repeat(REASON_MAX) + EMOJI);
      assert.equal(cut, "a".repeat(REASON_MAX));
      assert.equal(hasLoneSurrogate(cut), false, "a surrogate pair was split");
    });
  });
});
