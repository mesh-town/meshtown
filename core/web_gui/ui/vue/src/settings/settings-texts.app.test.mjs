/* `leaveHint` (the hint under "Leave network…" in Settings) inside the Android app (the user agent carries `MeshTownApp/`):
 * this phone is disconnected. On a desktop the word stays "computer". */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { leaveHint } from "./settings-texts.ts";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android";
const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/126.0.0.0 Safari/537.36";

describe("leaveHint", () => {
  describe("in the app", () => {
    it("names this phone and the network", () => {
      given(APP_UA);
      assert.equal(leaveHint("demo-network"), "Disconnect this phone from \"demo-network\" for good. You'll need a new invite to come back.");
    });

    it("a network with no name: \"the network\"", () => {
      given(APP_UA);
      assert.equal(leaveHint(""), "Disconnect this phone from the network for good. You'll need a new invite to come back.");
    });
  });

  describe("on a desktop", () => {
    it("names this computer and the network, as before", () => {
      given(DESKTOP_UA);
      assert.equal(leaveHint("demo-network"), "Disconnect this computer from \"demo-network\" for good. You'll need a new invite to come back.");
    });
  });
});

function given(userAgent) {
  vi.stubGlobal("navigator", { userAgent });
}
