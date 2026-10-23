/* Node API types: actions, network, token, error bodies. Types only; the shared header and rules are in `api-types.ts`. */

import type { ConfigDocument, ConfigFileInfo, FieldError } from "./api-types-config.ts";

// ---------------------------------------------------------------- actions and network

/** `POST /api/peer-addresses`, 200: partial success — not an error. */
export interface PeerAddressesAddResponse {
  added: number;
  rejected: { line: string; message: string }[];
}

/** `GET /api/network/key`: the whole key; `null` — there is no network. */
export interface NetworkKeyResponse {
  key: string | null;
}

/** `GET /api/network/invite`, 200; 409 `busy` while the node is not in a network. */
export interface InviteResponse {
  invite: string; // "mesh-invite2-…" (single-use, without the network key)
  holders?: number; // how many nodes besides the issuer can let a newcomer in (0..2)
  reach?: InviteReach; // absent on an old node — same as "spread"
}

/** Invite reach: who will be able to let a newcomer in. */
export type InviteReach = "issuer_only" | "issuer_and_one" | "spread";

/** Body of `POST /api/network/join`: a key, `mesh1-…` or an invite. */
export interface JoinRequest {
  key: string;
}

/** `POST /api/network/join`, 202. */
export interface JoinResponse {
  label: string | null;
  restart: boolean; // false for `waiting`
  /** The `mesh-invite2-` invite is accepted, the node is waiting (`node.invite_join`). */
  waiting?: boolean;
}

/** Body of `POST /api/network/create`. */
export interface CreateNetworkRequest {
  label: string;
}

/** `POST /api/network/create`, 200: the full key arrives only here. */
export interface CreateNetworkResponse {
  key: string;
  label: string;
  restart: true;
}

/** `POST /api/token/rotate`, 200. */
export interface TokenRotateResponse {
  token: string;
  url: string; // "http://127.0.0.1:<port>/?token=…"
}

// ---------------------------------------------------------------- errors

/** Error codes — docs/WEB-API.md §1.1, the code table. */
export type ApiErrorCode =
  | "bad_json"
  | "bad_request"
  | "bad_token"
  | "bad_host"
  | "bad_origin"
  | "not_found"
  | "bad_method"
  | "stale_rev"
  | "busy"
  | "too_large"
  | "invalid"
  | "invalid_key"
  | "not_available"
  | "rate_limited"
  | "stopping"
  | "config_write_failed"
  | "internal";

/** Body of any error — docs/WEB-API.md §1.1. Flat: `error` is a code string, not an object. */
export interface ApiErrorBody {
  error: ApiErrorCode;
  message: string;
}

/** 409 `stale_rev` on `PUT /api/config` — docs/WEB-API-config.md §1.7: the current revision and document. */
export interface StaleRevError extends ApiErrorBody {
  error: "stale_rev";
  rev: number;
  config: ConfigDocument;
}

/** 422 `invalid`: on `PUT /api/config` with `rev`; for actions `fields` is empty, for create — `label`. */
export interface InvalidError extends ApiErrorBody {
  error: "invalid";
  rev?: number; // only on `PUT /api/config`
  fields: FieldError[];
}

/** 422 `not_available`: turning on autostart or full tunnel — docs/WEB-API-config.md §1.7. */
export interface NotAvailableError extends ApiErrorBody {
  error: "not_available";
  fields: FieldError[];
}

/** 422 `invalid_key` on `POST /api/network/join`; `word` — only if it tripped on a word. */
export interface InvalidKeyError extends ApiErrorBody {
  error: "invalid_key";
  word?: string;
}

/** 500 `config_write_failed`: the file was not written, the body holds the file state. */
export interface ConfigWriteFailedError extends ApiErrorBody {
  error: "config_write_failed";
  file: ConfigFileInfo;
}

/** 429 `rate_limited` — reserved in the prose, the node does not return it now. */
export interface RateLimitedError extends ApiErrorBody {
  error: "rate_limited";
  retry_after_sec: number; // not confirmed by the code
}

/** Any API error body: `error` narrows it to a specific shape. */
export type ApiErrorResponse =
  | StaleRevError
  | InvalidError
  | NotAvailableError
  | InvalidKeyError
  | ConfigWriteFailedError
  | RateLimitedError
  | (ApiErrorBody & {
      error: Exclude<
        ApiErrorCode,
        "stale_rev" | "invalid" | "not_available" | "invalid_key" | "config_write_failed" | "rate_limited"
      >;
    });

/** Error body as the page reads it: the fields of the different `ApiErrorResponse` shapes at once, all optional
 * (`ApiError.body` is the parsed response JSON with no shape check). */
export type ApiErrorBodyLoose = Partial<ApiErrorBody> & {
  fields?: FieldError[];
  rev?: number;
  config?: ConfigDocument;
  word?: string;
  file?: ConfigFileInfo;
  retry_after_sec?: number;
};
