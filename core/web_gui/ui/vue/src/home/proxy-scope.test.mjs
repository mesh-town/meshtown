/* The remembered proxy scope: reading, writing, garbage and a missing storage. */

import { describe, expect, it } from "vitest";
import { loadAnswers, readProxyScope, saveBrowser, saveOs, writeProxyScope } from "./proxy-scope.ts";

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

  describe("the answers of the wizard", () => {
    const NONE = { scope: null, os: null, pick: null };

    it("nothing stored, no storage, a storage that throws: nothing chosen", () => {
      const broken = { getItem: () => { throw new Error("denied"); }, setItem: () => { throw new Error("denied"); } };
      expect(loadAnswers(memory())).toEqual(NONE);
      expect(loadAnswers(null)).toEqual(NONE);
      expect(loadAnswers(broken)).toEqual(NONE);
    });

    it("garbage in any field reads as not chosen, the other fields are kept", () => {
      const storage = memory({ "meshtown.proxyScope": "browser", "meshtown.proxyOs": "beos", "meshtown.proxyBrowser": "" });
      expect(loadAnswers(storage)).toEqual({ scope: "browser", os: null, pick: null });
    });

    it("the system and the browser written are read back", () => {
      const storage = memory();
      saveOs("windows", storage);
      saveBrowser("other", storage);
      writeProxyScope("system", storage);
      expect(loadAnswers(storage)).toEqual({ scope: "system", os: "windows", pick: "other" });
    });

    it("writing to no storage or to a storage that throws does not throw", () => {
      const broken = { getItem: () => null, setItem: () => { throw new Error("denied"); } };
      expect(() => saveOs("mac", null)).not.toThrow();
      expect(() => saveBrowser("safari", broken)).not.toThrow();
    });
  });
});
