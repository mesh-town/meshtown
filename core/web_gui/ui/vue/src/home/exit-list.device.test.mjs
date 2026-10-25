/* The "blocked" mark on an exit row names this device: "phone" inside the app, "computer" on a desktop. */

import { afterEach, describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { exitMetaText } from "./exit-list.ts";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android";
const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/126.0.0.0 Safari/537.36";
const given = (userAgent) => vi.stubGlobal("navigator", { userAgent });
afterEach(() => vi.unstubAllGlobals());
const blocked = { blocked: true, online: true, path: "direct", rttMs: 5, notExit: false };

describe("exitMetaText blocked mark", () => {
  it("desktop: this computer", () => {
    given(DESKTOP_UA);
    assert.equal(exitMetaText(blocked), "Blocked on this computer");
  });
  it("app: this phone", () => {
    given(APP_UA);
    assert.equal(exitMetaText(blocked), "Blocked on this phone");
  });
});
