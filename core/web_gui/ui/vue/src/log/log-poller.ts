/* The log polling core without Vue (tested in node): timer, `busy`, pause, merging responses.
 * Dependencies come as an argument — `api`, timers and state reading are replaced in the test. */

import type { ApiFn } from "../core/api.ts";
import type { LogResponse } from "../core/api-types.ts";
import { POLL_MS, emptyFeed, mergePoll, pollErrorText, pollPath } from "./log-view.ts";
import type { LogFeed } from "./log-view.ts";

/* deps: api(method, path), setTimer/clearTimer, active() — whether the tab with node data is open,
 * boot() — the current `boot_id`, net() — the selected network filter (`null` — all), max() — the ring size, onFeed(feed) — a new feed, onError(text) — the polling failure line ("" — no failure; optional). */
interface LogPollerDeps {
  api: ApiFn;
  setTimer: (fn: () => void, ms: number) => unknown;
  clearTimer: (id: unknown) => void;
  active: () => boolean;
  boot: () => string | null | undefined;
  net?: () => string | null;
  max: () => number;
  onFeed: (feed: LogFeed) => void;
  onError?: (text: string) => void;
}

interface LogPoller {
  start: () => void;
  stop: () => void;
  tick: () => Promise<void>;
  setPaused: (on: boolean) => void;
  reset: () => void;
  getFeed: () => LogFeed;
}

export function createLogPoller(deps: LogPollerDeps): LogPoller {
  let feed = emptyFeed();
  let paused = false;
  let timer: unknown = null;
  let busy = false;

  function setFeed(next: LogFeed): void {
    feed = next;
    deps.onFeed(feed);
  }

  function canTick(): boolean {
    return !paused && !busy && deps.active();
  }

  async function poll(): Promise<void> {
    const boot = deps.boot();
    const net = currentNet();
    const data = await deps.api<LogResponse | null>("GET", pollPath(feed.lastSeq, net));
    // the node restarted or the filter changed while the request was running: the response belongs to the old one
    if (deps.boot() !== boot || currentNet() !== net) return;
    setFeed(mergePoll(feed, data, deps.max()));
    report("");
  }

  function currentNet(): string | null {
    return deps.net ? deps.net() : null;
  }

  function report(text: string): void {
    if (deps.onError) deps.onError(text);
  }

  async function tick(): Promise<void> {
    if (!canTick()) return;
    busy = true;
    try {
      await poll();
    } catch (err) {
      // a network error (status 0) is not shown — the connection bar speaks of it; any other — a line above the window
      report(pollErrorText(err));
    } finally {
      busy = false;
    }
  }

  function start(): void {
    if (timer) return;
    timer = deps.setTimer(tick, POLL_MS);
    tick();
  }

  function stop(): void {
    deps.clearTimer(timer);
    timer = null;
  }

  function setPaused(on: boolean): void {
    paused = on;
    if (!on) tick();
  }

  function reset(): void {
    setFeed(emptyFeed());
  }

  return { start, stop, tick, setPaused, reset, getFeed: () => feed };
}
