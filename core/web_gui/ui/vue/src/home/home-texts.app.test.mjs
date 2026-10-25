/* Home texts about this device inside the app. */

import { afterEach, describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { HOME_JOIN_KEY_HINT, HOME_NO_RETURN_HINT } from "./home-card-texts.ts";
import { waitingStatusText } from "./home-flow-texts.ts";
import { exitServerHint } from "./home-words.ts";
import { NOTE_REMOVED } from "./invite-join-texts.ts";
import * as j from "./invite-join-texts.ts";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android";
const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/126.0.0.0 Safari/537.36";
const given = (userAgent) => vi.stubGlobal("navigator", { userAgent });
afterEach(() => vi.unstubAllGlobals());
const noComputer = (text) => assert.ok(!/computer/i.test(text), text);

describe("Home texts", () => {
  const cases = [
    ["HOME_JOIN_KEY_HINT", () => HOME_JOIN_KEY_HINT("family"), 'replaces "family" on this %s.'],
    ["HOME_NO_RETURN_HINT", () => HOME_NO_RETURN_HINT("family"), "This %s can't rejoin"],
    ["waitingStatusText", () => waitingStatusText(' "family"'), "let this %s in"],
    ["exitServerHint off", () => exitServerHint(false, null), "through this %s."],
    ["NOTE_REMOVED", () => NOTE_REMOVED("family"), "brings this %s back"],
  ];
  it.each(cases)("%s: phone in the app, computer on a desktop", (_n, make, part) => {
    given(APP_UA);
    assert.ok(make().includes(part.replace("%s", "phone")), make());
    noComputer(make());
    given(DESKTOP_UA);
    assert.ok(make().includes(part.replace("%s", "computer")), make());
  });

  it("the other invite notes", () => {
    given(APP_UA);
    for (const key of Object.keys(j).filter((k) => /^NOTE_/.test(k) && typeof j[k] === "function")) noComputer(j[key]("family"));
  });
});
