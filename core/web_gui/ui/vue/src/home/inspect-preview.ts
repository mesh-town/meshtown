/* What a pasted invite leads to, before it is used: `POST /api/network/inspect` after a pause in typing. A refusal, an older
 * node without the address and a node that is down all give no line: the join itself reports what is wrong. */

import { getCurrentScope, onScopeDispose, ref, watch } from "vue";
import type { Ref } from "vue";
import { api as defaultApi } from "../core/api.ts";
import type { ApiFn } from "../core/api.ts";
import type { InspectResponse } from "../core/api-types.ts";
import { previewManaged, previewPeer } from "./net-kind-texts.ts";

/** How long the field must stay unchanged before the node is asked. */
export const INSPECT_DELAY_MS = 500;

export interface InspectTimers {
  setTimer: (fn: () => void, ms: number) => unknown;
  clearTimer: (id: unknown) => void;
}

const pageTimers: InspectTimers = {
  setTimer: (fn, ms) => setTimeout(fn, ms),
  clearTimer: (id) => clearTimeout(id as ReturnType<typeof setTimeout>),
};

export async function inspectKey(api: ApiFn, key: string): Promise<InspectResponse | null> {
  try {
    return await api<InspectResponse>("POST", "/api/network/inspect", { key });
  } catch {
    return null;
  }
}

export const previewText = (reply: InspectResponse): string => (reply.kind === "Managed" ? previewManaged(reply.label) : previewPeer(reply.label));

/** `preview` is the one-line text for the string in the field, empty while there is none; a stale answer is dropped. */
export function useInspectPreview(getKey: () => string, api: ApiFn = defaultApi, timers: InspectTimers = pageTimers): { preview: Ref<string>; reply: Ref<InspectResponse | null> } {
  const preview = ref("");
  const reply = ref<InspectResponse | null>(null);
  let timer: unknown = null;
  let generation = 0;

  const cancel = (): void => {
    generation += 1;
    if (timer !== null) timers.clearTimer(timer);
    timer = null;
  };

  async function ask(key: string, mine: number): Promise<void> {
    timer = null;
    const answer = await inspectKey(api, key);
    if (mine !== generation) return;
    reply.value = answer;
    preview.value = answer ? previewText(answer) : "";
  }

  watch(getKey, (value) => {
    cancel();
    preview.value = "";
    reply.value = null;
    const key = value.trim();
    if (key) timer = timers.setTimer(() => void ask(key, generation), INSPECT_DELAY_MS);
  }, { immediate: true });
  if (getCurrentScope()) onScopeDispose(cancel);
  return { preview, reply };
}
