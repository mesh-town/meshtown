/* The block item of a managed network's row menu names the phone inside the app, the computer on a desktop. */

import { afterEach, describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { menuItems } from "./members-rows.ts";
import { peerOneRow } from "../testing/members-fixtures.mjs";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android";
const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/126.0.0.0 Safari/537.36";
const given = (userAgent) => vi.stubGlobal("navigator", { userAgent });
afterEach(() => vi.unstubAllGlobals());

const ME = "a".repeat(64);
const blockLabel = () => menuItems(peerOneRow(), { me: ME, managed: true, meAdmin: true }).find((i) => i.key === "block").label;

describe("managed row menu", () => {
  it("the app: the block item says phone", () => {
    given(APP_UA);
    assert.ok(blockLabel().includes("phone"), blockLabel());
    assert.ok(!/computer/i.test(blockLabel()), blockLabel());
  });

  it("a desktop: the block item says computer", () => {
    given(DESKTOP_UA);
    assert.ok(blockLabel().includes("computer"), blockLabel());
    assert.ok(!blockLabel().includes("phone"), blockLabel());
  });
});
