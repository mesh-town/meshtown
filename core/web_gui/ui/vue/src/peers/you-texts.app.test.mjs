/* `you-texts.ts` inside the Android app (the user agent carries `MeshTownApp/`): the "You" block says "phone" about this
 * device. On a desktop the words stay "computer". */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { youExitLabel, youLeaveHint } from "./you-texts.ts";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android";
const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/126.0.0.0 Safari/537.36";

describe("youLeaveHint", () => {
  it("in the app: disconnect this phone, the other networks stay", () => {
    given(APP_UA);
    assert.equal(youLeaveHint("my devices"), "Disconnect this phone from \"my devices\" for good. Your other networks stay. You'll need a new invite to come back.");
  });

  it("on a desktop: disconnect this computer, as before", () => {
    given(DESKTOP_UA);
    assert.equal(youLeaveHint("my devices"), "Disconnect this computer from \"my devices\" for good. Your other networks stay. You'll need a new invite to come back.");
  });
});

describe("youExitLabel", () => {
  it("in the app: members open sites through this phone", () => {
    given(APP_UA);
    assert.equal(youExitLabel("my devices"), "Let members of \"my devices\" open sites through this phone");
  });

  it("on a desktop: through this computer, as before", () => {
    given(DESKTOP_UA);
    assert.equal(youExitLabel("my devices"), "Let members of \"my devices\" open sites through this computer");
  });
});

function given(userAgent) {
  vi.stubGlobal("navigator", { userAgent });
}
