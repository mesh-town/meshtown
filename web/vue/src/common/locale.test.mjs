/* `locale.ts`: plural forms, number grouping and label ordering for the page locale. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { LOCALE, compareLabels, formatNumber, plural } from "./locale.ts";

describe("locale", () => {
  it("LOCALE is en-US", () => assert.equal(LOCALE, "en-US"));

  it("plural: exactly 1 is singular, everything else (0, 2, 11, 21) is plural", () => {
    assert.equal(plural(1, "member", "members"), "member");
    for (const n of [0, 2, 11, 21]) assert.equal(plural(n, "member", "members"), "members", String(n));
  });

  it("formatNumber groups thousands with a comma", () => assert.equal(formatNumber(12345), "12,345"));

  it("compareLabels orders alphabetically, not by character code", () => {
    assert.deepEqual(["b", "A", "a", "B"].sort(compareLabels), ["a", "A", "b", "B"]);
    assert.deepEqual(["Banana", "cherry", "apple"].sort(compareLabels), ["apple", "Banana", "cherry"]);
  });
});
