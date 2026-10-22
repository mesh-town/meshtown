/* Token exchange from the address before mounting: `temp` — storage is empty, the address is cleaned; failure — the token stays for the header. */

import { beforeEach, describe, expect, it, vi } from "vitest";
import { getToken } from "../core/token.ts";
import { defaultShellDeps } from "../shell/shell-deps.ts";
import { adoptAndExchangeToken } from "./token-exchange.ts";

function stubExchange(status, body) {
  const fetch = vi.fn(async () => new Response(JSON.stringify(body), { status }));
  vi.stubGlobal("fetch", fetch);
  return fetch;
}

beforeEach(() => {
  sessionStorage.clear();
  history.replaceState(null, "", "/?token=abc123#peers");
});

describe("adoptAndExchangeToken", () => {
  it("temporary token: POST /api/auth/token, sessionStorage is empty, the address has no token=", async () => {
    const fetch = stubExchange(200, { via: "temp" });
    await adoptAndExchangeToken(defaultShellDeps);
    expect(fetch.mock.calls[0][0]).toBe("/api/auth/token");
    expect(JSON.parse(fetch.mock.calls[0][1].body)).toEqual({ token: "abc123" });
    expect(getToken()).toBe("");
    expect(location.href).not.toContain("token=");
    expect(location.hash).toBe("#peers");
  });

  it("permanent token: the exchange succeeded — the token stays in sessionStorage for the header", async () => {
    stubExchange(200, { via: "token" });
    await adoptAndExchangeToken(defaultShellDeps);
    expect(getToken()).toBe("abc123");
    expect(location.href).not.toContain("token=");
  });

  it("exchange failed (old node, 404): not fatal — the token is in storage, the address is cleaned", async () => {
    stubExchange(404, { error: "not_found" });
    await adoptAndExchangeToken(defaultShellDeps);
    expect(getToken()).toBe("abc123");
    expect(location.href).not.toContain("token=");
  });

  it("network unavailable: not fatal either", async () => {
    vi.stubGlobal("fetch", async () => { throw new TypeError("Failed to fetch"); });
    await adoptAndExchangeToken(defaultShellDeps);
    expect(getToken()).toBe("abc123");
  });

  it("no token in the address: no requests, storage untouched", async () => {
    history.replaceState(null, "", "/");
    sessionStorage.setItem("meshtown.token", "old");
    const fetch = stubExchange(200, { via: "temp" });
    await adoptAndExchangeToken(defaultShellDeps);
    expect(fetch).not.toHaveBeenCalled();
    expect(getToken()).toBe("old");
  });
});
