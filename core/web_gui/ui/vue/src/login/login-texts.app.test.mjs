/* The "password not set yet" steps inside the app. */

import { afterEach, describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { stepRunText } from "./no-password-help.ts";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android";
const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/126.0.0.0 Safari/537.36";
const given = (userAgent) => vi.stubGlobal("navigator", { userAgent });
afterEach(() => vi.unstubAllGlobals());

describe("stepRunText", () => {
  it("in the app, on this device: the phone", () => {
    given(APP_UA);
    assert.equal(stepRunText("127.0.0.1"), "On this phone, run in a terminal:");
  });

  it("another host stays \"the computer running\"", () => {
    given(APP_UA);
    assert.equal(stepRunText("192.168.1.50"), "On 192.168.1.50 (the computer running Mesh Town), run this in a terminal:");
  });

  it("on a desktop: the computer", () => {
    given(DESKTOP_UA);
    assert.equal(stepRunText("localhost"), "On this computer, run in a terminal:");
  });
});
