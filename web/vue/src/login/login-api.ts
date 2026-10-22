/* Client of the `/api/auth/*` endpoints: status, token exchange, sign-in, sign-out, password. All requests go through one
 * helper with `credentials: "same-origin"` (session cookie). Not `core/api.ts`: a 401 on sign-in is not an "old token",
 * it must not wake the `meshtown:changed` bus. */

import { NOT_RESPONDING_TEXT } from "../common/words.ts";
import { getToken } from "../core/token.ts";
import { loginHttpErrorText } from "./login-texts.ts";

/** How the request credentials were obtained: `null` — not at all. */
export type AuthVia = "token" | "temp" | "password" | null;
export type AuthAccess = "localhost" | "localnet";

/** Body of `GET /api/auth/status`. */
export interface AuthStatus {
  password_set: boolean;
  access: AuthAccess;
  listening: AuthAccess;
  via: AuthVia;
}

/** Error of the sign-in endpoints: `status` 0 — the node did not respond; `retryAfterSec` — from `Retry-After` (429). */
export class AuthError extends Error {
  status: number;
  code: string;
  retryAfterSec: number | null;

  constructor(status: number, code: string, message: string, retryAfterSec: number | null = null) {
    super(message);
    this.name = "AuthError";
    this.status = status;
    this.code = code;
    this.retryAfterSec = retryAfterSec;
  }
}

function parseRetryAfterSec(res: Response): number | null {
  const sec = Number.parseInt(res.headers.get("Retry-After") || "", 10);
  return Number.isFinite(sec) && sec >= 0 ? sec : null;
}

async function readBody(res: Response): Promise<{ error?: string; message?: string } | null> {
  try {
    const text = await res.text();
    return text ? JSON.parse(text) : null;
  } catch (err) {
    return null;
  }
}

async function authErrorFromResponse(res: Response): Promise<AuthError> {
  const body = await readBody(res);
  const code = (body && body.error) || "http_" + res.status;
  return new AuthError(res.status, code, (body && body.message) || loginHttpErrorText(res.status), parseRetryAfterSec(res));
}

// The permanent token (if any) is also sent as a header: the password change accepts "a session or a token".
function requestInit(method: string, body: unknown): RequestInit {
  const headers: Record<string, string> = {};
  const init: RequestInit = { method, headers, credentials: "same-origin" };
  const token = getToken();
  if (token) headers["X-Meshtown-Token"] = token;
  if (body === undefined) return init;
  headers["Content-Type"] = "application/json";
  init.body = JSON.stringify(body);
  return init;
}

async function authFetch<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(path, requestInit(method, body)).catch(() => {
    throw new AuthError(0, "network", NOT_RESPONDING_TEXT);
  });
  if (!res.ok) throw await authErrorFromResponse(res);
  return (await readBody(res)) as T;
}

export const authStatus = (): Promise<AuthStatus> => authFetch<AuthStatus>("GET", "/api/auth/status");
export const exchangeToken = (token: string): Promise<{ via: "temp" | "token" }> => authFetch("POST", "/api/auth/token", { token });
export const login = (password: string): Promise<{ via: "password" }> => authFetch("POST", "/api/auth/login", { password });
export const logout = (): Promise<unknown> => authFetch("POST", "/api/auth/logout", {});
export const setPassword = (password: string): Promise<{ password_set: boolean }> => authFetch("PUT", "/api/auth/password", { password });
