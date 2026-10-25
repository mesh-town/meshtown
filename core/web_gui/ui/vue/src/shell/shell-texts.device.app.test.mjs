/* Shell texts about this device inside the app: the identity strip and the notices. */

import { afterEach, describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { identityStrip } from "./status-line.ts";
import { noticeText } from "./notice-texts.ts";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android";
const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/126.0.0.0 Safari/537.36";
const given = (userAgent) => vi.stubGlobal("navigator", { userAgent });
afterEach(() => vi.unstubAllGlobals());
const noComputer = (text) => assert.ok(!/computer/i.test(text), text);

const state = (reason) => ({ node: { status: "running", identity: "temporary", identity_reason: reason } });

describe("identityStrip", () => {
  it.each(["declined", "other"])("%s: the phone in the app, the computer on a desktop", (reason) => {
    given(APP_UA);
    noComputer(identityStrip(state(reason), null).text);
    given(DESKTOP_UA);
    assert.ok(identityStrip(state(reason), null).text.includes("this computer"));
  });

  it("a reason from the node is left as it is", () => {
    given(APP_UA);
    const text = identityStrip(state("declined"), { message: "no computer id here" }).text;
    assert.ok(text.startsWith("No computer id here. "), text);
    assert.ok(text.includes("as a new member"), text);
  });
});

describe("noticeText", () => {
  it("the phone has left the network", () => {
    given(APP_UA);
    assert.equal(noticeText({ id: 1, at_unix: 1, label: "family", by_name: "anna" }), '"family" was deleted by an admin (anna). This phone has left it.');
    given(DESKTOP_UA);
    assert.equal(noticeText({ id: 1, at_unix: 1, label: "family", by_name: "anna" }), '"family" was deleted by an admin (anna). This computer has left it.');
  });
});
