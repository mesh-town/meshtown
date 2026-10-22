/* Words of the "password not set yet" instructions: picking the variant by host. */

import { describe, expect, it } from "vitest";
import { isLocalHost, showRouterLine, stepRunText } from "./no-password-help.ts";

describe("no-password-help", () => {
  it("another host: step 1 names the host, the router line is present", () => {
    expect(stepRunText("192.168.1.50")).toBe("On 192.168.1.50 (the computer running Mesh Town), run this in a terminal:");
    expect(showRouterLine("192.168.1.50")).toBe(true);
  });

  it.each(["127.0.0.1", "localhost", "::1", "[::1]"])("local %s: \"On this computer\", no router", (host) => {
    expect(isLocalHost(host)).toBe(true);
    expect(stepRunText(host)).toBe("On this computer, run in a terminal:");
    expect(showRouterLine(host)).toBe(false);
  });
});
