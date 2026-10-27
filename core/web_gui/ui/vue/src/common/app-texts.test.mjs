/* Texts with a variant for the Android app: the phone wording and no pointer to the tray icon near the clock inside the app,
 * the desktop wording everywhere else. The choice is made on every call. */

import { afterEach, describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { API_ERROR_TEXT } from "../core/api.ts";
import { coreApiBadTokenText } from "../core/core-texts.ts";
import { secondAdminHintText, SECOND_ADMIN_HINT } from "../members/members-texts.ts";
import { STOPPED_NOTE, STOPPED_NOTE_APP } from "./pac-texts.ts";
import { settingsTokenDescText, SETTINGS_TOKEN_DESC, stoppedNoteText } from "../settings/settings-texts.ts";
import { shellBadOriginText, SHELL_BAD_ORIGIN } from "../shell/shell-texts.ts";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android";
const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/126.0.0.0 Safari/537.36";
const given = (userAgent) => vi.stubGlobal("navigator", { userAgent });
afterEach(() => vi.unstubAllGlobals());
const clean = (text) => assert.ok(!/\bcomputer\b|near the clock|tray icon/i.test(text), text);

describe("app variants of the texts", () => {
  it("the second-admin hint: phone in the app, computer on a desktop", () => {
    given(APP_UA);
    clean(secondAdminHintText());
    assert.ok(secondAdminHintText().includes("if this phone is lost"));
    given(DESKTOP_UA);
    assert.equal(secondAdminHintText(), SECOND_ADMIN_HINT);
  });

  it("the wrong-address strip: no tray icon in the app, the icon near the clock on a desktop", () => {
    given(APP_UA);
    clean(shellBadOriginText());
    assert.ok(shellBadOriginText().includes("app"));
    given(DESKTOP_UA);
    assert.equal(shellBadOriginText(), SHELL_BAD_ORIGIN);
  });

  it("the old-link message: open the app, not the icon near the clock", () => {
    given(APP_UA);
    clean(coreApiBadTokenText());
    clean(API_ERROR_TEXT.bad_token);
    given(DESKTOP_UA);
    assert.ok(coreApiBadTokenText().includes("icon near the clock"));
    assert.ok(API_ERROR_TEXT.bad_token.includes("icon near the clock"));
  });

  it("the token description: no tray icon in the app, the tray icon on a desktop", () => {
    given(APP_UA);
    clean(settingsTokenDescText());
    given(DESKTOP_UA);
    assert.equal(settingsTokenDescText(), SETTINGS_TOKEN_DESC);
  });

  it("the note under Stop: the app variant in the app, the clock variant on a desktop", () => {
    given(APP_UA);
    assert.equal(stoppedNoteText(), STOPPED_NOTE_APP);
    clean(STOPPED_NOTE_APP);
    given(DESKTOP_UA);
    assert.equal(stoppedNoteText(), STOPPED_NOTE);
  });
});
