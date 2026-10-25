/* Refusal texts about this device inside the app. */

import { afterEach, describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { ADMISSION_REFUSED_TITLE, admissionRefusedAsk } from "./peers-texts.ts";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android";
const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/126.0.0.0 Safari/537.36";
const given = (userAgent) => vi.stubGlobal("navigator", { userAgent });
afterEach(() => vi.unstubAllGlobals());
const noComputer = (text) => assert.ok(!/computer/i.test(text), text);

describe("the refused-admission plate", () => {
  it("in the app: the phone", () => {
    given(APP_UA);
    assert.equal(ADMISSION_REFUSED_TITLE("family"), 'This phone can\'t connect to "family"');
    assert.ok(admissionRefusedAsk(["anna"], "family", false).startsWith('Ask anna to let this phone in.'));
    assert.ok(admissionRefusedAsk([], "family", true).startsWith('Ask an admin of "family" to let this phone in.'));
    noComputer(admissionRefusedAsk(["a", "b"], "family", false));
    noComputer(admissionRefusedAsk([], "family", false));
  });

  it("on a desktop: the computer", () => {
    given(DESKTOP_UA);
    assert.equal(ADMISSION_REFUSED_TITLE("family"), 'This computer can\'t connect to "family"');
    assert.ok(admissionRefusedAsk(["anna"], "family", false).startsWith("Ask anna to let this computer in."));
    assert.ok(admissionRefusedAsk([], "family", true).startsWith('Ask an admin of "family" to let this computer in.'));
  });
});
