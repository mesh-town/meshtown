/* The pure logic of the Log section: time label, filter, merging polling responses, the line ring.
 * The cases are moved from the classic `11-log.test.mjs` (deleted together with `11-log.js`). */

import { formatNumber } from "../common/locale.ts";
import { errReason, lowerFirst } from "../common/words.ts";
import { LOG_GAP_TEXT, LOG_POLL_FAILED_PREFIX, LOG_POLL_FAILED_RETRY, LOG_RING_HINT_HEAD, LOG_RING_HINT_TAIL } from "./log-texts.ts";
import type { LogLine, LogResponse, LogSummary } from "../core/api-types.ts";

/** The feed gap: the program erased some lines before the page read them. */
export interface LogGap {
  gap: true;
}
export type LogEntry = (LogLine & { gap?: undefined }) | LogGap;
/** An entry with a key for `v-for`. */
export type KeyedEntry = LogEntry & { key: string };
/** The page feed: lines and the `seq` of the last one read. */
export interface LogFeed {
  lines: KeyedEntry[];
  lastSeq: number;
}

export const POLL_MS = 1000;
export const DEFAULT_LOG_RING_LINES = 2000;
const LOG_POLL_LIMIT_LINES = 500;
const TIME_WIDTH = 7;
export const GAP_TEXT = LOG_GAP_TEXT;

export function formatLogTime(t: number): string {
  return "+" + t.toFixed(1).padStart(TIME_WIDTH, " ") + "s";
}

export function entryPasses(entry: LogEntry, tag: string, search: string): boolean {
  if (entry.gap) return true;
  if (tag !== "all" && entry.tag !== tag) return false;
  return !search || entry.text.toLowerCase().includes(search.toLowerCase());
}

export function visibleEntries<T extends LogEntry>(lines: T[], tag: string, search: string): T[] {
  return lines.filter((entry) => entryPasses(entry, tag, search));
}

export function ringHint(log: Pick<LogSummary, "ring_lines"> | null | undefined): string {
  const n = (log && log.ring_lines) || DEFAULT_LOG_RING_LINES;
  return LOG_RING_HINT_HEAD + formatNumber(n) + LOG_RING_HINT_TAIL;
}

// The line above the window on a polling failure; a network error (status 0) is not shown — the connection bar speaks of it.
export function pollErrorText(err: unknown): string {
  if ((err as { status?: number } | null)?.status === 0) return "";
  const reason = errReason(err);
  return reason ? LOG_POLL_FAILED_PREFIX + lowerFirst(reason) : LOG_POLL_FAILED_RETRY;
}

export function chipTags(log: Pick<LogSummary, "tags"> | null | undefined): string[] {
  return (log && log.tags) || [];
}

export function logRingMaxLines(log: Pick<LogSummary, "ring_lines"> | null | undefined): number {
  return (log && log.ring_lines) || DEFAULT_LOG_RING_LINES;
}

export function pollPath(lastSeq: number): string {
  return "/api/log?after=" + lastSeq + "&limit=" + LOG_POLL_LIMIT_LINES;
}

export function downloadUrl(token: string): string {
  return "/api/log/download?token=" + encodeURIComponent(token);
}

// The key of a line for `v-for`: an entry has `seq`, a gap does not.
function keyed(entry: LogEntry): KeyedEntry {
  return entry.gap ? { gap: true, key: "gap" } : { ...entry, key: "l" + entry.seq };
}

// The (one-based) index of the n-th real line; a gap is not counted.
function nthRealIndex(lines: LogEntry[], n: number): number {
  let seen = 0;
  return lines.findIndex((entry) => !entry.gap && ++seen === n);
}

function cutIndex(lines: LogEntry[], max: number): number {
  const over = lines.filter((entry) => !entry.gap).length - max;
  if (over <= 0) return 0;
  return nthRealIndex(lines, over) + 1;
}

// The ring: beyond `max` lines (a gap is not counted) the oldest entries are dropped.
export function trimRing<T extends LogEntry>(lines: T[], max: number): T[] {
  return lines.slice(cutIndex(lines, max));
}

// `data.gap` — some lines were pushed out: the feed starts anew with a gap.
function mergeLines(feed: LogFeed, data: LogResponse, max: number): KeyedEntry[] {
  const base: KeyedEntry[] = data.gap ? [keyed({ gap: true })] : feed.lines;
  const incoming = data.lines || [];
  if (!incoming.length) return base;
  return trimRing(base.concat(incoming.map(keyed)), max);
}

function nextSeq(feed: LogFeed, data: LogResponse): number {
  const incoming = data.lines || [];
  if (incoming.length) return incoming[incoming.length - 1].seq;
  return data.last_seq != null ? data.last_seq : feed.lastSeq;
}

// A `GET /api/log` response → the new feed state `{lines, lastSeq}`; `null` in the response — the state is the same.
export function mergePoll(feed: LogFeed, data: LogResponse | null | undefined, max: number): LogFeed {
  if (!data) return feed;
  return { lines: mergeLines(feed, data, max), lastSeq: nextSeq(feed, data) };
}

export function emptyFeed(): LogFeed {
  return { lines: [], lastSeq: 0 };
}
