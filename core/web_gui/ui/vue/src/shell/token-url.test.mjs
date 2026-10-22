/* `token-url.ts`: the token from the page address and from a pasted address. */

import { describe, it } from "vitest";
import assert from "node:assert/strict";
import { TOKEN_KEY, getToken } from "../core/token.ts";
import { adoptUrlToken, takeTokenFromUrl, tokenFromPasted } from "./token-url.ts";

describe("takeTokenFromUrl", () => {
  it("token removed, path and hash preserved", () => {
    const got = takeTokenFromUrl("http://127.0.0.1:7070/?token=abc#peers");
    assert.deepEqual(got, { token: "abc", cleanUrl: "/#peers" });
  });

  it("other parameters stay", () => {
    const got = takeTokenFromUrl("http://h/p?a=1&token=abc&b=2#log");
    assert.equal(got.token, "abc");
    assert.equal(got.cleanUrl, "/p?a=1&b=2#log");
  });

  it("no token — null", () => {
    assert.equal(takeTokenFromUrl("http://h/?a=1#log"), null);
  });
});

describe("tokenFromPasted", () => {
  it("full address with a token", () => {
    assert.equal(tokenFromPasted("http://127.0.0.1:7070/?token=xyz", "http://127.0.0.1:7070/"), "xyz");
  });

  it("a relative address is resolved against the base", () => {
    assert.equal(tokenFromPasted("/?token=rel", "http://127.0.0.1:7070/"), "rel");
  });

  it("garbage, an address without a token and an empty string — null", () => {
    assert.equal(tokenFromPasted("http://[junk", "http://h/"), null, "garbage");
    assert.equal(tokenFromPasted("http://h/?a=1", "http://h/"), null, "no token");
    assert.equal(tokenFromPasted("", "http://h/"), null, "empty");
  });
});

describe("adoptUrlToken", () => {
  it("puts the token into storage and replaces the address with a clean one once", () => {
    const w = setup("http://h/?token=abc#peers");
    adoptUrlToken(w.deps);
    assert.equal(getToken(w.storage), "abc");
    assert.deepEqual(w.replaced, ["/#peers"]);
  });

  it("no token in the address — storage and address untouched", () => {
    const w = setup("http://h/#peers");
    adoptUrlToken(w.deps);
    assert.equal(w.storage.data.has(TOKEN_KEY), false);
    assert.deepEqual(w.replaced, []);
  });
});

function setup(href) {
  const data = new Map();
  const storage = { data, getItem: (k) => data.get(k) ?? null, setItem: (k, v) => data.set(k, v) };
  const replaced = [];
  return { storage, replaced, deps: { href, storage, replaceUrl: (url) => replaced.push(url) } };
}
