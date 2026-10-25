/* The page's connection with the node — pure state without Vue:
 * the failure counter, poll backoff, the current status strip. It is changed only by poll requests and
 * `refreshConfig`. */

export const POLL_VISIBLE_MS = 2000;
export const POLL_HIDDEN_MS = 10000;
export const BACKOFF_STEPS = [2000, 2000, 5000, 15000];
// While the node is stopped, polling is rare: the node may have been started again.
export const POLL_STOPPED_MS = 15000;

import { SHELL_BAD_ORIGIN, SHELL_CHANGED, SHELL_RESTARTED, SHELL_STOPPED, SHELL_STOPPED_APP } from "./shell-texts.ts";
import { API_ERROR_TEXT } from "../core/api.ts";
import type { ApiErrorBodyLoose } from "../core/api-types.ts";
import type { Absence } from "../core/store.ts";
import { inAppPage } from "../home/browser-setup.ts";

const TEXT_DISCONNECTED = API_ERROR_TEXT.network;
const TEXT_RESTARTED = SHELL_RESTARTED;
const TEXT_BAD_TOKEN = API_ERROR_TEXT.bad_token;
const TEXT_BAD_ORIGIN = SHELL_BAD_ORIGIN;
const TEXT_CHANGED = SHELL_CHANGED;

type StripKind = "disconnected" | "restarted" | "bad_token" | "bad_origin" | "stopped";
export type StripViewKind = StripKind | "changed";
export interface Strip<K extends string = StripKind> {
  kind: K;
  text: string;
}

/** The connection: the failure counter, backoff, the strip. */
export interface Conn {
  failures: number;
  backoffMs: number;
  stopped: boolean;
  awaitingToken: boolean;
  strip: Strip | null;
  configChanged: boolean;
  bootId: string | null; // boot_id of the last snapshot: a change means the node restarted
  sawFirst: boolean;
}

type ErrorKind = "token" | "origin" | "stopping" | "failure" | "alive";

export function createConn(): Conn {
  return {
    failures: 0,
    backoffMs: POLL_VISIBLE_MS,
    stopped: false,
    awaitingToken: false,
    strip: null,
    configChanged: false,
    bootId: null,
    sawFirst: false,
  };
}

export function nextBackoff(failures: number): number {
  if (failures <= 0) return POLL_VISIBLE_MS;
  return BACKOFF_STEPS[Math.min(failures, BACKOFF_STEPS.length - 1)];
}

function setStrip(conn: Conn, kind: StripKind, text: string): void {
  conn.strip = { kind, text };
}

function quiet(conn: Conn): boolean {
  return conn.stopped || conn.awaitingToken;
}

function showDisconnected(conn: Conn): void {
  if (conn.failures >= 2 && !quiet(conn)) setStrip(conn, "disconnected", TEXT_DISCONNECTED);
}

export function onFailure(conn: Conn): void {
  conn.failures++;
  conn.backoffMs = nextBackoff(conn.failures);
  showDisconnected(conn);
}

// The "No connection" strip only exists at `failures >= 2`, a separate counter check is not needed.
function wasDisconnected(conn: Conn): boolean {
  return conn.strip?.kind === "disconnected";
}

function clearStoppedStrip(conn: Conn): void {
  if (conn.strip?.kind !== "stopped") return;
  conn.strip = null;
}

// The first successful response after "No connection" is "restarted" (the caller clears the strip on a timer).
export function onSuccess(conn: Conn): "restarted" | null {
  const restarted = wasDisconnected(conn);
  conn.failures = 0;
  conn.backoffMs = POLL_VISIBLE_MS;
  clearStoppedStrip(conn);
  conn.stopped = false;
  if (restarted) setStrip(conn, "restarted", TEXT_RESTARTED);
  return restarted ? "restarted" : null;
}

export function onBadToken(conn: Conn): void {
  conn.awaitingToken = true;
  setStrip(conn, "bad_token", TEXT_BAD_TOKEN);
}

// To the console only on the first entry into "bad_origin" (polling repeats the response every couple of seconds).
export function onBadOrigin(conn: Conn, body?: ApiErrorBodyLoose | null): void {
  const first = conn.strip?.kind !== "bad_origin";
  if (first && body && body.message) console.warn("Mesh Town:", body.message);
  setStrip(conn, "bad_origin", TEXT_BAD_ORIGIN);
}

export function onStopping(conn: Conn): void {
  conn.stopped = true;
  setStrip(conn, "stopped", inAppPage() ? SHELL_STOPPED_APP : SHELL_STOPPED);
}

const BY_STATUS = new Map<number, ErrorKind>([
  [401, "token"],
  [403, "origin"],
  [503, "stopping"],
]);

// By `ApiError.status`: 0 or not an `ApiError` — a failure; any other HTTP response — the node is alive.
export function classify(err: unknown): ErrorKind {
  const status = (err as { status?: number } | null | undefined)?.status;
  if (!status) return "failure";
  return BY_STATUS.get(status) || "alive";
}

// Why the page gets no snapshots; the order as in the header (`status-line.ts::absent`).
export function absence(conn: Conn): Absence | null {
  if (conn.awaitingToken) return "token";
  if (conn.strip?.kind === "disconnected") return "silent";
  return conn.stopped ? "stopped" : null;
}

// The strip above the page: the connection outranks "Settings changed in another tab".
export function stripView(conn: Conn): Strip<StripViewKind> | null {
  if (conn.strip) return { kind: conn.strip.kind, text: conn.strip.text };
  return conn.configChanged ? { kind: "changed", text: TEXT_CHANGED } : null;
}
