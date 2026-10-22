/* Polling `/api/state` and loading the config without Vue (a port of `04-poll.js`): intervals, backoff
 * on failures, reacting to `boot_id` and `config_rev` changes. It writes only to the passed `store`/`conn`. */

import type { ChangedWhat } from "../core/bridge.ts";
import type { ApiErrorBodyLoose, ConfigMeta, ConfigResponse, NodeState } from "../core/api-types.ts";
import type { Store } from "../core/store.ts";
import type { Conn } from "./conn.ts";
import type { ShellDeps } from "./shell-deps.ts";
import { POLL_HIDDEN_MS, POLL_STOPPED_MS, classify, onBadOrigin, onBadToken, onFailure, onStopping, onSuccess } from "./conn.ts";
import { fetchMembers } from "../members/members-poll.ts";
import { defaultShellDeps } from "./shell-deps.ts";

const RESTART_NOTICE_MS = 4000;

type Reaction = (conn: Conn, err: unknown) => void;

const REACTION_BY_ERROR_KIND = new Map<string, Reaction>([
  ["token", (conn) => onBadToken(conn)],
  ["origin", (conn, err) => onBadOrigin(conn, (err as { body?: ApiErrorBodyLoose | null }).body)],
  ["stopping", (conn) => onStopping(conn)],
  ["failure", (conn) => onFailure(conn)],
]);

const CONFIG_META_LISTS = ["read_only", "restart_fields", "restart_pending", "cli_overrides"] as const;

function configMeta(data: ConfigResponse): ConfigMeta {
  const meta: Partial<ConfigMeta> = {};
  for (const key of CONFIG_META_LISTS) meta[key] = data[key] || [];
  return meta as ConfigMeta;
}

// The revision only grows within one boot; a restart is handled by the config reload. While the node is
// starting, a newer revision is its own write and a restart is ahead: the next poll after the start decides.
function foreignRevision(store: Store, data: NodeState, restarted: boolean): boolean {
  if (store.configRev === null || restarted) return false;
  return data.config_rev > store.configRev && data.node.status !== "starting";
}

/** Polling the state and config. */
export interface Poller {
  start: () => Promise<void>;
  schedule: (delay: number) => void;
  clearPollTimer: () => void;
  onBus: (what: string | undefined) => void;
  pollOnce: () => Promise<void>;
  refreshConfig: () => Promise<void>;
  stop: () => void;
  stoppedRecently: () => boolean;
}

export function createPoller(store: Store, conn: Conn, deps: ShellDeps = defaultShellDeps): Poller {
  let generation = 0; // grows on stop and token: the response of a request started earlier is discarded
  let timer: unknown = null;
  let noticeTimer: unknown = null;
  let inFlight = false;
  let pollAgainAfterCurrent = false;
  let closed = false;
  let stoppedAt: number | null = null; // when the node was stopped from this page; cleared by any live answer

  const blocked = () => closed || conn.awaitingToken;
  const tokenNow = () => (deps.getToken ? deps.getToken() : "");
  // 401 for a token that has been replaced by the time of the response: the request was with the old one, the connection is not spoiled.
  const staleAuth = (err: unknown, asked: string): boolean => (err as { status?: number } | null)?.status === 401 && tokenNow() !== asked;
  // A stopped node is polled once per POLL_STOPPED_MS, a hidden tab does not lengthen that.
  const nextDelay = () => (conn.stopped ? POLL_STOPPED_MS : deps.isHidden() ? POLL_HIDDEN_MS : conn.backoffMs);

  function schedule(delay: number): void {
    deps.clearTimer(timer);
    timer = null;
    if (blocked()) return;
    if (inFlight) {
      pollAgainAfterCurrent = pollAgainAfterCurrent || delay === 0;
      return;
    }
    timer = deps.setTimer(pollOnce, delay);
  }

  function hideRestartNotice() {
    if (conn.strip && conn.strip.kind === "restarted") conn.strip = null;
  }

  function markAlive() {
    stoppedAt = null;
    if (onSuccess(conn) !== "restarted") return;
    deps.clearTimer(noticeTimer);
    noticeTimer = deps.setTimer(hideRestartNotice, RESTART_NOTICE_MS);
  }

  // Classification of a poll request error: the connection changes by the rules of `conn.ts`.
  function noteError(err: unknown): void {
    const kind = classify(err);
    if (kind === "alive") return markAlive();
    REACTION_BY_ERROR_KIND.get(kind)?.(conn, err);
  }

  async function refreshConfig(): Promise<void> {
    const born = generation;
    const asked = tokenNow();
    try {
      const data = await deps.api<ConfigResponse | null>("GET", "/api/config");
      if (born !== generation) return;
      markAlive();
      if (!data) return;
      store.config = data.config;
      store.configRev = data.rev;
      store.configMeta = configMeta(data);
      store.configFile = data.file || null;
      conn.configChanged = false;
    } catch (err) {
      if (born === generation && !staleAuth(err, asked)) noteError(err); // the error itself is swallowed: the config is pulled in at the next case
    }
  }

  async function acceptSnapshot(data: NodeState): Promise<void> {
    const restarted = conn.bootId !== null && data.boot_id !== conn.bootId;
    if (restarted) refreshConfig();
    conn.bootId = data.boot_id;
    if (foreignRevision(store, data, restarted)) conn.configChanged = true;
    if (!conn.sawFirst) {
      conn.sawFirst = true;
  // The config comes before the first snapshot write: the identity strip takes the reason text from the `file` block.
      if (!store.configRev) await refreshConfig();
    }
    store.state = data;
  }

  async function fetchState(): Promise<void> {
    const born = generation;
    const asked = tokenNow();
    try {
      const data = await deps.api<NodeState | null>("GET", "/api/state");
      if (born !== generation) return;
      markAlive();
      if (data) await acceptSnapshot(data);
      if (data && born === generation) await fetchMembers(store, deps.api); // never throws, never touches `conn`
    } catch (err) {
      if (born === generation && !staleAuth(err, asked)) noteError(err);
    }
  }

  async function pollOnce(): Promise<void> {
    if (inFlight || blocked()) return;
    inFlight = true;
    try {
      await fetchState();
    } finally {
      inFlight = false;
    }
    const again = pollAgainAfterCurrent;
    pollAgainAfterCurrent = false;
    schedule(again ? 0 : nextDelay());
  }

  function start(): Promise<void> {
    closed = false;
    return pollOnce();
  }

  function clearPollTimer() {
    deps.clearTimer(timer);
    timer = null;
  }

  const REACTION_BY_BUS_EVENT: Partial<Record<ChangedWhat, () => void>> = {
    config: () => refreshConfig(),
    state: () => schedule(0),
    token: () => stopQuietly(onBadToken),
    stopped: () => noteStopped(),
    stopping: () => noteStopping(),
  };

  // A stop is about to be requested: the pending poll and the response in flight are dropped, no strip yet.
  function noteStopping() {
    clearPollTimer();
    generation++;
    pollAgainAfterCurrent = false;
  }

  // The response in flight is discarded; polling doesn't stand still but runs rarely (`POLL_STOPPED_MS`).
  function noteStopped() {
    stoppedAt = Date.now();
    generation++;
    pollAgainAfterCurrent = false;
    onStopping(conn);
    schedule(nextDelay());
  }

  function stopQuietly(setState: (c: Conn) => void): void {
    generation++;
    setState(conn);
    clearPollTimer();
  }

  // A `meshtown:changed` bus event (`what`); an unknown one is silently ignored.
  function onBus(what: string | undefined): void {
    const reaction = REACTION_BY_BUS_EVENT[what as ChangedWhat];
    if (reaction) reaction();
  }

  function stop() {
    closed = true;
    deps.clearTimer(timer);
    deps.clearTimer(noticeTimer);
    timer = null;
    noticeTimer = null;
  }

  // Within POLL_STOPPED_MS of a stop the node may still answer 200 while exiting: a poll now would undo "stopped".
  const stoppedRecently = () => stoppedAt !== null && Date.now() - stoppedAt < POLL_STOPPED_MS;

  return { start, schedule, clearPollTimer, onBus, pollOnce, refreshConfig, stop, stoppedRecently };
}
