/* A row with the vote of this device: the second line names the phone inside the app, the computer on a desktop. */

import { afterEach, describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { rowDecor } from "./members-rows.ts";
import { peerOneRow, voteInfo } from "../testing/members-fixtures.mjs";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android";
const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/126.0.0.0 Safari/537.36";
const given = (userAgent) => vi.stubGlobal("navigator", { userAgent });
afterEach(() => vi.unstubAllGlobals());

const ME = "a".repeat(64);
const secondLine = () => {
  const row = peerOneRow({ vote: voteInfo() });
  return rowDecor({ id_full: row.id_full }, row, ME).lines[1];
};

describe("rowDecor with vote.mine", () => {
  it("the app: Blocked on this phone", () => {
    given(APP_UA);
    assert.equal(secondLine(), "Blocked on this phone");
  });

  it("a desktop: Blocked on this computer", () => {
    given(DESKTOP_UA);
    assert.equal(secondLine(), "Blocked on this computer");
  });
});
