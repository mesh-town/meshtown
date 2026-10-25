/* The "Home" panel when the node is silent, inside the Android app (the user agent carries `MeshTownApp/`): the hint does not
 * point to a tray icon (a phone has none) but says to open the app. `token` has no hint, as on a desktop. On a desktop the
 * hint about the icon near the clock stays, and `ABSENCE_HINT` is still exported with it. */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { ABSENCE_HINT, START_HINT_APP } from "../common/words.ts";
import { statePanel } from "./home-view.ts";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android";
const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/126.0.0.0 Safari/537.36";
const HINT_APP = "Start it: open the Mesh Town app.";
const HINT_DESKTOP = "Start it: the Mesh Town icon near the clock → \"Start Mesh Town\".";

describe("the Home panel when the node does not answer", () => {
  describe("in the app", () => {
    it("silent: the grey panel says to open the app", () => {
      given(APP_UA);
      const panel = statePanel(snap(), "silent");
      assert.deepEqual(panel, { level: "grey", main: "Mesh Town is not responding", line: "Mesh Town is not responding", sub: HINT_APP, actions: [] });
    });

    it("stopped: the grey panel says to open the app", () => {
      given(APP_UA);
      const panel = statePanel(snap(), "stopped");
      assert.equal(panel.main, "Mesh Town is stopped");
      assert.equal(panel.sub, HINT_APP);
    });

    it("token: no hint, as before", () => {
      given(APP_UA);
      assert.equal(statePanel(snap(), "token").sub, "");
    });

    it("the hint never mentions the icon or the clock", () => {
      given(APP_UA);
      for (const absent of ["silent", "stopped"]) {
        const sub = statePanel(snap(), absent).sub;
        assert.ok(!/icon|clock/.test(sub), sub);
      }
    });
  });

  describe("on a desktop", () => {
    it.each(["silent", "stopped"])("%s: the hint about the icon near the clock, as before", (absent) => {
      given(DESKTOP_UA);
      assert.equal(statePanel(snap(), absent).sub, HINT_DESKTOP);
    });

    it("token: no hint", () => {
      given(DESKTOP_UA);
      assert.equal(statePanel(snap(), "token").sub, "");
    });
  });

  describe("the constants", () => {
    it("START_HINT_APP is the sentence for the app", () => {
      assert.equal(START_HINT_APP, HINT_APP);
    });

    it("ABSENCE_HINT keeps the desktop wording and an empty token", () => {
      assert.deepEqual(ABSENCE_HINT, { token: "", silent: HINT_DESKTOP, stopped: HINT_DESKTOP });
    });
  });
});

function given(userAgent) {
  vi.stubGlobal("navigator", { userAgent });
}

function snap() {
  return {
    node: { status: "online", id: "aaaa1111" },
    network: { label: "home", id_hex: "net-a" },
    peers: [{ self: true, online: true, id: "aaaa1111", id_full: "A".repeat(64), name: "me" }],
    route: { state: "direct", exits: [] },
  };
}
