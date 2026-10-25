/* `conn.ts::onStopping` inside the Android app (the user agent carries `MeshTownApp/`): the "stopped" strip tells to open
 * the app instead of the tray icon. The choice is made when the strip is set, not when the module loads. The wrong-origin and
 * bad-token strips keep their wording: the app opens the page itself. */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { SHELL_STOPPED_APP } from "./shell-texts.ts";
import { createConn, onBadOrigin, onBadToken, onStopping } from "./conn.ts";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android";
const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/126.0.0.0 Safari/537.36";
const STOPPED_APP = "Mesh Town is stopped — start it: open the Mesh Town app. You don't need to refresh the page.";
const STOPPED_DESKTOP = "Mesh Town is stopped — start it: the Mesh Town icon near the clock → \"Start Mesh Town\". You don't need to refresh the page.";

describe("onStopping", () => {
  it("in the app: the strip says to open the Mesh Town app", () => {
    given(APP_UA);
    const conn = createConn();
    onStopping(conn);
    assert.equal(conn.stopped, true);
    assert.deepEqual(conn.strip, { kind: "stopped", text: STOPPED_APP });
  });

  it("in the app: the strip does not mention the icon or the clock", () => {
    given(APP_UA);
    const conn = createConn();
    onStopping(conn);
    assert.ok(!/icon|clock/.test(conn.strip.text), conn.strip.text);
  });

  it("on a desktop: the strip points to the icon near the clock, as before", () => {
    given(DESKTOP_UA);
    const conn = createConn();
    onStopping(conn);
    assert.deepEqual(conn.strip, { kind: "stopped", text: STOPPED_DESKTOP });
  });

  it("the page agent is read at the call: one module, both answers", () => {
    given(DESKTOP_UA);
    const first = createConn();
    onStopping(first);
    given(APP_UA);
    const second = createConn();
    onStopping(second);
    assert.deepEqual([first.strip.text, second.strip.text], [STOPPED_DESKTOP, STOPPED_APP]);
  });
});

describe("the other strips in the app", () => {
  it("onBadOrigin keeps the wording about the icon", () => {
    given(APP_UA);
    const conn = createConn();
    vi.spyOn(console, "warn").mockImplementation(() => {});
    onBadOrigin(conn, null);
    assert.equal(conn.strip.text, "The page was opened from the wrong address — open it from the Mesh Town icon near the clock");
  });

  it("onBadToken keeps its wording", () => {
    given(APP_UA);
    const conn = createConn();
    onBadToken(conn);
    assert.equal(conn.strip.kind, "bad_token");
    assert.ok(conn.strip.text.includes("icon near the clock"), conn.strip.text);
  });
});

describe("SHELL_STOPPED_APP", () => {
  it("is the sentence for the app", () => {
    assert.equal(SHELL_STOPPED_APP, STOPPED_APP);
  });
});

function given(userAgent) {
  vi.stubGlobal("navigator", { userAgent });
}
