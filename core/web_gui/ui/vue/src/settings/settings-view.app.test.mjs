/* The caption of the web access field in several networks, inside the app. */

import { afterEach, describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { MAIN_FIELDS, EXPERT_FIELDS, PAGE_FIELDS } from "./settings-fields.ts";
import { fieldLabel } from "./settings-view.ts";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android";
const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/126.0.0.0 Safari/537.36";
const given = (userAgent) => vi.stubGlobal("navigator", { userAgent });
afterEach(() => vi.unstubAllGlobals());

const field = [...MAIN_FIELDS, ...EXPERT_FIELDS, ...PAGE_FIELDS].find((f) => f.path === "web.access");

describe("fieldLabel of web.access with several networks", () => {
  it("in the app: the phone", () => {
    given(APP_UA);
    assert.equal(fieldLabel(field, true), "Which connections of this phone can open this page (Wi-Fi, cable)");
  });

  it("on a desktop: the computer", () => {
    given(DESKTOP_UA);
    assert.equal(fieldLabel(field, true), "Which connections of this computer can open this page (Wi-Fi, cable)");
  });
});
