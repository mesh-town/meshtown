/* `excludedTitle` in the Android app (the user agent carries `MeshTownApp/`): "phone"; on a desktop the word stays "computer". */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { excludedTitle } from "./members-texts.ts";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android";
const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/126.0.0.0 Safari/537.36";

describe("excludedTitle", () => {
  it("in the app: banned, forgotten and removed name the phone; left has no device word", () => {
    vi.stubGlobal("navigator", { userAgent: APP_UA });
    assert.equal(excludedTitle("banned", "net"), "This phone was banned from the network \"net\"");
    assert.equal(excludedTitle("forgotten", "net"), "This phone was removed from the member list \"net\"");
    assert.equal(excludedTitle("banned", "net", true), "This phone was removed from the network \"net\"");
    assert.equal(excludedTitle("left", "net"), "You left the network \"net\"");
  });

  it("on a desktop: the titles are as before", () => {
    vi.stubGlobal("navigator", { userAgent: DESKTOP_UA });
    assert.equal(excludedTitle("banned", "net"), "This computer was banned from the network \"net\"");
    assert.equal(excludedTitle("forgotten", "net"), "This computer was removed from the member list \"net\"");
    assert.equal(excludedTitle("banned", "net", true), "This computer was removed from the network \"net\"");
  });
});
