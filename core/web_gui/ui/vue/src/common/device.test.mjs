/* `forDevice(text)`: in the Android app (the user agent carries `MeshTownApp/`) the word "computer" becomes "phone";
 * on a desktop the text is returned as is. The choice is made on every call, not when the module loads. */

import { describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { forDevice } from "./device.ts";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android";
const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/126.0.0.0 Safari/537.36";

describe("forDevice", () => {
  describe("on a desktop", () => {
    it("the text with \"computer\" stays as it is", () => {
      given(DESKTOP_UA);
      assert.equal(forDevice("Disconnect this computer from the network."), "Disconnect this computer from the network.");
    });

    it("a text without the word stays as it is", () => {
      given(DESKTOP_UA);
      assert.equal(forDevice("Leave network"), "Leave network");
    });

    it("no navigator at all (a plain node run) counts as a desktop", () => {
      vi.stubGlobal("navigator", undefined);
      assert.equal(forDevice("this computer"), "this computer");
    });
  });

  describe("in the app", () => {
    it("\"computer\" becomes \"phone\"", () => {
      given(APP_UA);
      assert.equal(forDevice("Disconnect this computer from the network."), "Disconnect this phone from the network.");
    });

    it("every occurrence is replaced", () => {
      given(APP_UA);
      assert.equal(forDevice("this computer, that computer"), "this phone, that phone");
    });

    it("the word inside another word is left alone", () => {
      given(APP_UA);
      assert.equal(forDevice("computers and supercomputer"), "computers and supercomputer");
    });

    it("the word next to punctuation and a quote is replaced", () => {
      given(APP_UA);
      assert.equal(forDevice("(computer) \"computer\", computer."), "(phone) \"phone\", phone.");
    });

    it("a text without the word stays as it is", () => {
      given(APP_UA);
      assert.equal(forDevice("Leave network"), "Leave network");
    });

    it("an empty text stays empty", () => {
      given(APP_UA);
      assert.equal(forDevice(""), "");
    });
  });

  it("the user agent is read at call time: the same text gives both answers in one run", () => {
    const text = "this computer";
    given(DESKTOP_UA);
    const onDesktop = forDevice(text);
    given(APP_UA);
    const inApp = forDevice(text);
    assert.deepEqual([onDesktop, inApp], ["this computer", "this phone"]);
  });
});

function given(userAgent) {
  vi.stubGlobal("navigator", { userAgent });
}
