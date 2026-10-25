/* Polling `/api/overview` (or `/api/state` + `/api/members` on an old node) and loading the config without Vue:
 * intervals, backoff on failures, reacting to `boot_id` and `config_rev` changes. It writes only to the passed `store`/`conn`. */

import type { ChangedWhat } from "../core/bridge.ts";
import type { ApiErrorBodyLoose, ConfigResponse, NodeState } from "../core/api-types.ts";
import type { Store } from "../core/store.ts";
import type { Conn } from "./conn.ts";
import type { ShellDeps } from "./shell-deps.ts";
import { POLL_HIDDEN_MS, POLL_STOPPED_MS, classify, onBadOrigin, onBadToken, onFailure, onStopping, onSuccess } from "./conn.ts";
import { ApiError } from "../core/api.ts";
import { acceptOverview, legacyOverview, parseOverview } from "../core/overview.ts";
import { fetchMembers } from "../members/members-poll.ts";
import { configMeta } from "../core/config-meta.ts";
import { readSlotConfig } from "./slot-config.ts";
import { defaultShellDeps } from "./shell-deps.ts";

const RESTART_NOTICE_MS = 4000;

type Reaction = (conn: Conn, err: unknown) => void;

const REACTION_BY_ERROR_KIND = new Map<string, Reaction>([
  ["token", (conn) => onBadToken(conn)],
  ["origin", (conn, err) => onBadOrigin(conn, (err as { body?: ApiErrorBodyLoose | null }).body)],
  ["stopping", (conn) => onStopping(conn)],
  ["failure", (conn) => onFailure(conn)],
]);

// Only an old node answers the overview route with "not found": the node's own 404 body carries this code.
const isNoOverview = (err: unknown): boolean => err instanceof ApiError && err.status === 404 && err.code === "not_found";

// The revision only grows within one boot; a restart is handled by the config reload. While the node is
// starting, a newer revision is its own write and a restart is ahead: the next poll after the start decides.
function foreignRevision(store: Store, data: NodeState, restarted: boolean): boolean {
  if (store.configRev === null || restarted) return false;
  return data.config_rev > store.configRev && data.node.status !== "starting";
}

/** Polling the state and config. */
interface Poller {
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
  let mode: "overview" | "legacy" = "overview"; // legacy — the node has no `/api/overview`, until its `boot_id` changes
  let stoppedAt: number | null = null; // when the node was stopped from this page; cleared by any live answer

  const blocked = () => closed || conn.awaitingToken;
  const tokenNow = () => (deps.getToken ? deps.getToken() : "");
  // 401 for a token that has been replaced by the time of the response: the request was with the old one, the connection is not spoiled.
  const staleAuth = (err: unknown, asked: string): boolean => (err as { status?: number } | null)?.status === 401 && tokenNow() !== asked;
  // A stopped node is polled once per POLL_STOPPED_MS, a hidden tab does not lengthen that.
  const nextDelay = () => (conn.stopped ? POLL_STOPPED_MS : deps.isHidden() ? POLL_HIDDEN_MS : conn.backoffMs);

  function restartTimer(delay: number): void {
    deps.clearTimer(timer);
    timer = null;
    if (blocked()) return;
    timer = deps.setTimer(pollOnce, delay);
  }

  function schedule(delay: number): void {
    if (inFlight) {
      deps.clearTimer(timer);
      timer = null;
      if (!blocked()) pollAgainAfterCurrent = pollAgainAfterCurrent || delay === 0;
      return;
    }
    restartTimer(delay);
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

  // The main slot's snapshot: reactions to `boot_id` and `config_rev`. True — the node restarted since the last one.
  async function observe(data: NodeState): Promise<boolean> {
    const restarted = conn.bootId !== null && data.boot_id !== conn.bootId;
    if (restarted) refreshConfig();
    conn.bootId = data.boot_id;
    if (foreignRevision(store, data, restarted)) conn.configChanged = true;
    if (!conn.sawFirst) {
      conn.sawFirst = true;
      // The config comes before the first snapshot write: the identity strip takes the reason text from the `file` block.
      if (!store.configRev) await refreshConfig();
    }
    return restarted;
  }

  async function readSlotConfigs(ids: string[], born: number): Promise<void> {
    for (const id of ids) {
      if (born !== generation) return;
      await readSlotConfig(store, deps.api, id, store.slots[id]?.state?.boot_id ?? null);
    }
  }

  async function fetchOverview(): Promise<void> {
    const born = generation;
    const asked = tokenNow();
    try {
      const ov = parseOverviewReply(await deps.api<unknown>("GET", "/api/overview"), born);
      if (!ov) return;
      const main = ov.slots.find((s) => s.slot === "main")?.state ?? null;
      if (main) await observe(main); // no main state: observe is skipped on purpose, a boot_id change waits for the next live overview
      if (born !== generation) return;
      store.source = "overview";
      await readSlotConfigs(acceptOverview(store, ov), born);
    } catch (err) {
      if (born !== generation) return;
      if (isNoOverview(err)) {
        mode = "legacy";
        return fetchState();
      }
      if (!staleAuth(err, asked)) noteError(err);
    }
  }

  // A live answer of a request started in the current generation; null — discarded or not an overview.
  function parseOverviewReply(raw: unknown, born: number) {
    if (born !== generation) return null;
    markAlive();
    return parseOverview(raw);
  }

  async function fetchState(): Promise<void> {
    const born = generation;
    const asked = tokenNow();
    try {
      const data = await deps.api<NodeState | null>("GET", "/api/state");
      if (born !== generation) return;
      markAlive();
      if (!data) return;
      if (await observe(data)) mode = "overview";
      if (born !== generation) return;
      store.source = "legacy";
      acceptOverview(store, legacyOverview(data, store.members));
      await fetchMembers(store, deps.api); // never throws, never touches `conn`
      if (born === generation) acceptOverview(store, legacyOverview(data, store.members));
    } catch (err) {
      if (born === generation && !staleAuth(err, asked)) noteError(err);
    }
  }

  async function pollOnce(): Promise<void> {
    if (inFlight || blocked()) return;
    inFlight = true;
    try {
      await (mode === "overview" ? fetchOverview() : fetchState());
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
