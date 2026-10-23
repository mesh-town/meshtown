/* The live log feed: polling `GET /api/log` while the Log tab is open and there is no pause; the feed is reset
 * on a new node `boot_id` (the node restarted — the log started anew). Core — `log-poller.ts`. */

import { onUnmounted, ref, shallowRef, watch } from "vue";
import type { Ref, ShallowRef } from "vue";
import { api } from "../core/api.ts";
import type { ApiFn } from "../core/api.ts";
import type { Store } from "../core/store.ts";
import { createLogPoller } from "./log-poller.ts";
import { effectiveLogNet, emptyFeed, logRingMaxLines } from "./log-view.ts";
import type { LogFeed } from "./log-view.ts";

export interface LogFeedState {
  feed: ShallowRef<LogFeed>;
  paused: Ref<boolean>;
  pollError: Ref<string>;
  togglePause: () => void;
}

// The log belongs to the process: its start is the main slot's, whichever network leads.
function processBoot(store: Store): string | null | undefined {
  return store.slots.main?.bootId ?? (store.state && store.state.boot_id);
}

export function useLogFeed(store: Store, net: Ref<string | null> = ref<string | null>(null)): LogFeedState {
  const feed = shallowRef<LogFeed>(emptyFeed());
  const paused = ref<boolean>(false);
  const pollError = ref<string>("");
  const poller = createLogPoller({
    api: api as ApiFn,
    setTimer: (fn: () => void, ms: number) => setInterval(fn, ms),
    clearTimer: (id: unknown) => clearInterval(id as ReturnType<typeof setInterval> | undefined),
    active: () => store.activeSection === "log",
    boot: () => processBoot(store),
    net: () => effectiveLogNet(store, net.value),
    max: () => logRingMaxLines(store.state && store.state.log),
    onFeed: (next: LogFeed) => (feed.value = next),
    onError: (text: string) => (pollError.value = text),
  });

  function togglePause(): void {
    paused.value = !paused.value;
    poller.setPaused(paused.value);
  }

  watch(() => store.activeSection === "log" && !!store.state, (on) => (on ? poller.start() : poller.stop()), { immediate: true });
  watch(() => effectiveLogNet(store, net.value), () => {
    poller.reset();
    poller.tick();
  });
  watch(() => processBoot(store), (now, prev) => {
    if (prev != null && now !== prev) poller.reset();
  });
  onUnmounted(poller.stop);

  return { feed, paused, pollError, togglePause };
}
