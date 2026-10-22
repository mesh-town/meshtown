/* `token.ts`: reading the token from session storage; the key is shared with the classic page. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { getToken, setToken, TOKEN_KEY } from "./token.ts";

describe("getToken", () => {
  it("storage key is the same as the classic page's: meshtown.token", () => {
    assert.equal(TOKEN_KEY, "meshtown.token");
  });

  it("token present — returns the value", () => {
    const storage = { getItem: (k) => (k === "meshtown.token" ? "abc" : null) };
    assert.equal(getToken(storage), "abc");
  });

  it("no token — empty string", () => {
    assert.equal(getToken({ getItem: () => null }), "");
  });

  it("storage throws — empty string, not an exception", () => {
    const storage = {
      getItem() {
        throw new Error("SecurityError");
      },
    };
    assert.equal(getToken(storage), "");
  });
});

describe("setToken", () => {
  it("writes to storage under the key meshtown.token", () => {
    const writes = [];
    setToken("new-1", { setItem: (k, v) => writes.push([k, v]) });
    assert.deepEqual(writes, [["meshtown.token", "new-1"]]);
  });

  it("getToken reads what was written", () => {
    const data = new Map();
    const storage = { setItem: (k, v) => data.set(k, v), getItem: (k) => data.get(k) ?? null };
    setToken("abc", storage);
    assert.equal(getToken(storage), "abc");
  });

  it("storage throws — no exception", () => {
    const storage = {
      setItem() {
        throw new Error("QuotaExceededError");
      },
    };
    assert.doesNotThrow(() => setToken("abc", storage));
  });
});
