/* Home texts about this device inside the Android app (the user agent carries `MeshTownApp/`): `managedWarning` and
 * `reachWarning` say "phone". The constants `HOME_REACH_*` stay the desktop wording; on a desktop nothing changes. */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { HOME_REACH_ISSUER_AND_ONE, HOME_REACH_ISSUER_ONLY } from "./home-flow-texts.ts";
import { reachWarning } from "./invite-actions.ts";
import { managedWarning } from "./net-kind-texts.ts";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android";
const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/126.0.0.0 Safari/537.36";

describe("managedWarning", () => {
  it("in the app: if this phone's data is lost, with the typed name", () => {
    given(APP_UA);
    assert.equal(
      managedWarning("work"),
      'If this phone\'s Mesh Town data is lost and nobody else is an admin, "work" can\'t be managed any more. Make a second admin soon.',
    );
  });

  it("in the app: without a name — \"this network\"", () => {
    given(APP_UA);
    assert.equal(
      managedWarning("  "),
      "If this phone's Mesh Town data is lost and nobody else is an admin, this network can't be managed any more. Make a second admin soon.",
    );
  });

  it("on a desktop: this computer's data, as before", () => {
    given(DESKTOP_UA);
    assert.equal(
      managedWarning("work"),
      'If this computer\'s Mesh Town data is lost and nobody else is an admin, "work" can\'t be managed any more. Make a second admin soon.',
    );
  });
});

describe("reachWarning", () => {
  describe("in the app", () => {
    it("issuer_only: only this phone can let them in", () => {
      given(APP_UA);
      assert.equal(reachWarning("issuer_only"), "Only this phone can let them in — don't turn it off until the new member joins.");
    });

    it("issuer_and_one: better not to turn this phone off", () => {
      given(APP_UA);
      assert.equal(reachWarning("issuer_and_one"), "Better not to turn this phone off until the new member joins.");
    });

    it("spread and an unknown value: still no warning", () => {
      given(APP_UA);
      assert.equal(reachWarning("spread"), "");
      assert.equal(reachWarning("everyone"), "");
    });
  });

  describe("on a desktop", () => {
    it("issuer_only: only this computer, as before", () => {
      given(DESKTOP_UA);
      assert.equal(reachWarning("issuer_only"), "Only this computer can let them in — don't turn it off until the new member joins.");
    });

    it("issuer_and_one: this computer, as before", () => {
      given(DESKTOP_UA);
      assert.equal(reachWarning("issuer_and_one"), "Better not to turn this computer off until the new member joins.");
    });
  });

  it("the HOME_REACH_* constants keep the desktop wording", () => {
    given(APP_UA);
    assert.equal(HOME_REACH_ISSUER_ONLY, "Only this computer can let them in — don't turn it off until the new member joins.");
    assert.equal(HOME_REACH_ISSUER_AND_ONE, "Better not to turn this computer off until the new member joins.");
  });
});

function given(userAgent) {
  vi.stubGlobal("navigator", { userAgent });
}
