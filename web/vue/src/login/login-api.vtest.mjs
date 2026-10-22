/* `login-api.ts`: the `/api/auth/*` endpoints through the shared fetch helper; errors carry a code and the seconds from `Retry-After`. */

import { describe, expect, it, vi } from "vitest";
import { AuthError, authStatus, exchangeToken, login, logout, setPassword } from "./login-api.ts";

function stub(status, body, headers = {}) {
  const calls = [];
  vi.stubGlobal("fetch", async (url, init) => {
    calls.push({ url, init });
    return new Response(body === null ? null : JSON.stringify(body), { status, headers });
  });
  return calls;
}

describe("login-api", () => {
  it("authStatus: GET /api/auth/status, same-origin, status body", async () => {
    const status = { password_set: true, access: "localnet", listening: "localnet", via: null };
    const calls = stub(200, status);
    expect(await authStatus()).toEqual(status);
    expect(calls[0].url).toBe("/api/auth/status");
    expect(calls[0].init.method).toBe("GET");
    expect(calls[0].init.credentials).toBe("same-origin");
  });

  it("exchangeToken, login, logout, setPassword: method, path and body", async () => {
    const calls = stub(200, { via: "temp" });
    await exchangeToken("tok");
    await login("password-1234");
    await logout();
    await setPassword("new-password");
    const seen = calls.map((c) => [c.init.method, c.url, c.init.body]);
    expect(seen).toEqual([
      ["POST", "/api/auth/token", '{"token":"tok"}'],
      ["POST", "/api/auth/login", '{"password":"password-1234"}'],
      ["POST", "/api/auth/logout", "{}"],
      ["PUT", "/api/auth/password", '{"password":"new-password"}'],
    ]);
    expect(calls.every((c) => c.init.credentials === "same-origin")).toBe(true);
  });

  it("429: code, status and the seconds from Retry-After", async () => {
    stub(429, { error: "too_many_attempts", message: "x" }, { "Retry-After": "42" });
    const err = await login("a").catch((e) => e);
    expect(err).toBeInstanceOf(AuthError);
    expect([err.status, err.code, err.retryAfterSec]).toEqual([429, "too_many_attempts", 42]);
  });

  it("401 and 409: code from the body, no seconds", async () => {
    stub(401, { error: "bad_password", message: "x" });
    const bad = await login("a").catch((e) => e);
    expect([bad.status, bad.code, bad.retryAfterSec]).toEqual([401, "bad_password", null]);
    stub(409, { error: "no_password", message: "x" });
    const none = await login("a").catch((e) => e);
    expect([none.status, none.code]).toEqual([409, "no_password"]);
  });

  it("network unavailable: status 0, code network", async () => {
    vi.stubGlobal("fetch", async () => { throw new TypeError("Failed to fetch"); });
    const err = await authStatus().catch((e) => e);
    expect(err).toBeInstanceOf(AuthError);
    expect([err.status, err.code]).toEqual([0, "network"]);
  });
});
