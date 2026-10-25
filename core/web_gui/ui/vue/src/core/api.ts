/* api() — a fetch wrapper with a token and a timeout (a port of `js/03-api.js`). The status strips
 * are drawn by the shell poll (`shell/poller.js`); from the core only a 401 ("token") goes to the bridge. */

import type { ApiErrorBodyLoose } from "./api-types.ts";
import { notifyChanged } from "./bridge.ts";
import { getToken } from "./token.ts";
import { NOT_RESPONDING_TEXT } from "../common/words.ts";
import { CORE_API_BAD_TOKEN, CORE_API_NO_CONFIG, CORE_API_TIMEOUT, coreApiHttpText } from "./core-texts.ts";

// Texts for people: the code in `ApiError.code` stays machine-readable, `message` holds an English phrase.
export const API_ERROR_TEXT = {
  timeout: CORE_API_TIMEOUT,
  network: NOT_RESPONDING_TEXT,
  bad_token: CORE_API_BAD_TOKEN,
  no_config: CORE_API_NO_CONFIG,
  http: coreApiHttpText,
};

export class ApiError extends Error {
  status: number;
  code: string;
  body: ApiErrorBodyLoose | null;

  constructor(status: number, code: string, message: string = code, body: ApiErrorBodyLoose | null = null) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
    this.body = body;
  }
}

// Structural shape of an error: `ApiError` or any object with the same fields (test stubs, foreign `Error`s).
export interface ApiErrorLike {
  message: string;
  code?: string;
  body?: ApiErrorBodyLoose | null;
}

export interface ApiDeps {
  fetch: (input: string, init: RequestInit) => Promise<Response>;
  getToken: () => string;
  timeoutMs: number;
  busTarget: EventTarget | undefined; // where to send `meshtown:changed`; no DOM — undefined
}

// Signature of `api()` for injection: a 204 response and an empty body give null, the caller sets the response type.
export type ApiFn = <T = unknown>(method: string, path: string, body?: unknown) => Promise<T>;

// Globals are read at call time, not at import: tests run in node without a DOM.
export const defaultApiDeps: ApiDeps = {
  fetch: (input, init) => fetch(input, init),
  getToken: () => getToken(),
  timeoutMs: 5000,
  get busTarget() {
    return typeof document === "undefined" ? undefined : document;
  },
};

function requestInit(token: string, method: string, body: unknown, signal: AbortSignal): RequestInit {
  const headers: Record<string, string> = { "X-Meshtown-Token": token };
  const init: RequestInit = { method, headers, signal };
  if (body != null) {
    headers["Content-Type"] = "application/json";
    init.body = JSON.stringify(body);
  }
  return init;
}

// Race between fetch and a timer: a stub fetch deaf to the signal doesn't hang the call.
async function send(d: ApiDeps, path: string, init: RequestInit, controller: AbortController): Promise<Response> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      controller.abort();
      reject(new ApiError(0, "timeout", API_ERROR_TEXT.timeout));
    }, d.timeoutMs);
  });
  try {
    return await Promise.race([d.fetch(path, init), timeout]);
  } catch (err) {
    throw err instanceof ApiError ? err : new ApiError(0, "network", API_ERROR_TEXT.network);
  } finally {
    clearTimeout(timer);
  }
}

async function readJson(res: Response): Promise<any> { // any: the response body is parsed JSON, the caller sets the shape
  try {
    const text = await res.text();
    return text ? JSON.parse(text) : null;
  } catch (err) {
    return null;
  }
}

// A 401 for a token that has already been replaced (for example "Revoke" in this tab) doesn't wake the bus: the page already has the new one.
function notifyTokenRejected(d: ApiDeps, sentToken: string): void {
  if (d.busTarget && d.getToken() === sentToken) notifyChanged(d.busTarget, "token");
}

async function failure(d: ApiDeps, res: Response, sentToken: string): Promise<ApiError> {
  const body = await readJson(res);
  if (res.status === 401) {
    notifyTokenRejected(d, sentToken);
    return new ApiError(401, "bad_token", API_ERROR_TEXT.bad_token, body);
  }
  const code = (body && body.error) || "http_" + res.status;
  return new ApiError(res.status, code, (body && body.message) || API_ERROR_TEXT.http(res.status), body);
}

export async function api<T = unknown>(method: string, path: string, body?: unknown, deps: Partial<ApiDeps> = defaultApiDeps): Promise<T> {
  const d: ApiDeps = Object.assign({}, defaultApiDeps, deps);
  const controller = new AbortController();
  const sentToken = d.getToken();
  const res = await send(d, path, requestInit(sentToken, method, body, controller.signal), controller);
  if (!res.ok) throw await failure(d, res, sentToken);
  return res.status === 204 ? (null as T) : readJson(res);
}
