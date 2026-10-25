/* `joinNetwork` inside the app: the "waiting" status of a key without a network label names the phone. */

import { afterEach, describe, it, vi } from "vitest";
import assert from "node:assert/strict";
import { JOIN_WAITING, joinNetwork } from "./join-actions.ts";
import { setup } from "../testing/join-setup.mjs";

const APP_UA = "Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android";
const DESKTOP_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/126.0.0.0 Safari/537.36";
const given = (userAgent) => vi.stubGlobal("navigator", { userAgent });
afterEach(() => vi.unstubAllGlobals());

async function waitingStatus() {
  const w = setup({ reply: { label: null, restart: false, waiting: true } });
  w.form.inviteInput = "mesh-invite2-abc";
  await joinNetwork(w.form, w.store, w.deps);
  return w.form.status;
}

describe("joinNetwork, waiting without a label", () => {
  it("the app says phone", async () => {
    given(APP_UA);
    const status = await waitingStatus();
    assert.ok(status.includes("let this phone in"), status);
    assert.ok(!/computer/i.test(status), status);
  });

  it("the desktop keeps JOIN_WAITING", async () => {
    given(DESKTOP_UA);
    assert.equal(await waitingStatus(), JOIN_WAITING);
    assert.ok(JOIN_WAITING.includes("let this computer in"));
  });
});
