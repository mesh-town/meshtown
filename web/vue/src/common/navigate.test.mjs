/* `navigate.ts`: navigating to an address through a replaceable `loc`. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { navigateTo } from "./navigate.ts";

describe("navigateTo", () => {
  it("calls assign on the stub loc with this address", () => {
    const seen = [];
    navigateTo("/api/log/download?token=t", { assign: (url) => seen.push(url) });
    assert.deepEqual(seen, ["/api/log/download?token=t"]);
  });
});
