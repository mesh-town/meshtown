import { describe, expect, it } from "vitest";
import { compareVersions, newestVersion, parseVersion } from "./update-version.ts";

describe("update-version", () => {
  it("compares numerically part by part", () => {
    expect(compareVersions("0.3.10", "0.3.9")).toBe(1);
    expect(compareVersions("0.3.9", "0.3.10")).toBe(-1);
    expect(compareVersions("1.0", "1.0.0")).toBe(0);
    expect(compareVersions("v0.4.0", "0.3.28")).toBe(1);
  });

  it("returns null for garbage", () => {
    expect(compareVersions("abc", "0.1.0")).toBeNull();
    expect(compareVersions("0.1.0", null)).toBeNull();
    expect(parseVersion("1.2.x")).toBeNull();
    expect(parseVersion("")).toBeNull();
  });

  it("picks the newest valid version", () => {
    expect(newestVersion(["0.1.0", "junk", null, "v0.2.5", "0.2.10"])).toBe("0.2.10");
    expect(newestVersion(["junk", null])).toBeNull();
  });
});
