/* The `main.ts` entry point as a whole (the wiring that the `mountPage` helper bypasses: it calls `adoptUrlToken`, while `main` calls
 * `adoptAndExchangeToken`): an address with `?token=` → exchange on the node → a cleaned address, the token in storage, the page
 * came up in `#app`; then a 401 on polling → the sign-in screen (the address part). */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { getToken } from "../core/token.ts";
import { demoConfig, demoState } from "../testing/demo-node.mjs";
import { createFakeNode } from "../testing/fake-node.mjs";
import { settle } from "../testing/page.mjs";

const STEP_MS = 500;
let root;

beforeEach(() => {
  vi.useFakeTimers();
  vi.resetModules();
  sessionStorage.clear();
  history.replaceState(null, "", "/?token=old-token");
  root = document.createElement("div");
  root.id = "app";
  document.body.appendChild(root);
});
afterEach(() => {
  root.remove();
  vi.unstubAllGlobals();
  vi.useRealTimers();
  history.replaceState(null, "", "/");
});

async function advanceUntil(done, limitMs) {
  for (let spent = 0; !done() && spent <= limitMs; spent += STEP_MS) {
    await vi.advanceTimersByTimeAsync(STEP_MS);
    await settle();
  }
}

describe("main.ts", () => {
  it("?token= in the address: exchange on the node, the address without token=, the page in #app; 401 on polling — the sign-in screen", async () => {
    const routes = { "POST /api/auth/token": { status: 200, body: { via: "token" } } };
    const node = createFakeNode({ state: demoState(), config: demoConfig(), routes });
    vi.stubGlobal("fetch", node.fetch);

    await import("../main.ts");
    await advanceUntil(() => root.querySelector('[data-ready="true"]') !== null, 10000);

    expect(node.count("POST", "/api/auth/token")).toBe(1);
    expect(node.calls.find((c) => c.path === "/api/auth/token").body).toEqual({ token: "old-token" });
    expect(location.href).not.toContain("token=");
    expect(getToken()).toBe("old-token");
    expect(root.querySelector("#status-line")).not.toBeNull();

    node.fail = { path: "/api/state", status: 401, body: { error: "bad_token" } };
    await advanceUntil(() => root.querySelector("#login-form") !== null, 15000);
    expect(root.querySelector("#login-form")).not.toBeNull();
    expect(root.querySelector("#status-line")).toBeNull();
  });
});
