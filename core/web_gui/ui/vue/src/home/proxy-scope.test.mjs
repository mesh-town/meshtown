/* The remembered proxy scope: reading, writing, garbage and a missing storage. */

import { describe, expect, it } from "vitest";
import { readProxyScope, writeProxyScope } from "./proxy-scope.ts";

const memory = (initial = {}) => {
  const data = { ...initial };
  return { data, getItem: (k) => (k in data ? data[k] : null), setItem: (k, v) => { data[k] = v; } };
};

describe("proxy scope", () => {
  it("nothing stored: not chosen", () => {
    expect(readProxyScope(memory())).toBeNull();
  });

  it.each(["system", "browser"])("%s: written and read back", (scope) => {
    const storage = memory();
    writeProxyScope(scope, storage);
    expect(readProxyScope(storage)).toBe(scope);
  });

  it.each(["", "System", "both", "1"])("garbage %j: not chosen", (raw) => {
    expect(readProxyScope(memory({ "meshtown.proxyScope": raw }))).toBeNull();
  });

  it("no storage: not chosen, writing does not throw", () => {
    expect(readProxyScope(null)).toBeNull();
    expect(() => writeProxyScope("system", null)).not.toThrow();
  });

  it("a storage that throws: not chosen", () => {
    const broken = { getItem: () => { throw new Error("denied"); }, setItem: () => { throw new Error("denied"); } };
    expect(readProxyScope(broken)).toBeNull();
    expect(() => writeProxyScope("browser", broken)).not.toThrow();
  });
});
