/* Wrapper of the join card: form, computed texts and actions. The logic is in `join-actions.ts`. */

import { computed, inject, reactive, ref } from "vue";
import type { ComputedRef, Ref } from "vue";
import type { Store } from "../core/store.ts";
import { defaultHomeDeps } from "./home-actions.ts";
import type { HomeDeps } from "./home-actions.ts";
import { createNetwork, createJoinForm, highlightParts, joinNetwork } from "./join-actions.ts";
import type { JoinForm, JoinResult, HighlightPart } from "./join-actions.ts";
import { JOIN_PROMISE_MS } from "./join-actions.ts";
import { inviteFromSearch, withoutInvite } from "./invite-param.ts";
import { joinProgress } from "./join-progress.ts";
import type { JoinProgress, JoinSent } from "./join-progress.ts";
import { HOME_KEY } from "./use-home.ts";
import type { HomeContext } from "./use-home.ts";
import { HOME_BUTTON_JOIN, HOME_BUTTON_SWITCH, HOME_TITLE_CREATE, HOME_TITLE_JOIN, HOME_TITLE_SWITCH } from "./home-flow-texts.ts";

export interface JoinView {
  form: JoinForm;
  hasNet: ComputedRef<boolean>;
  showJoin: ComputedRef<boolean>;
  showCreate: ComputedRef<boolean>;
  title: ComputedRef<string>;
  buttonText: ComputedRef<string>;
  errorParts: ComputedRef<HighlightPart[]>;
  progress: ComputedRef<JoinProgress | null>;
  join: () => Promise<JoinResult>;
  create: () => Promise<JoinResult>;
  openJoin: () => void;
  openCreate: () => void;
  toggleCreate: () => void;
}

// "Join sent" holds until the node answers with a new start; the deadline insures against an endless spinner
// (waiting for members takes longer: its progress is shown by the node itself via `invite_join`).
const SENT_WAITING_MS = 10 * 60 * 1000;

function markSent(res: JoinResult, sent: Ref<JoinSent | null>, store: Store, deps: HomeDeps): JoinResult {
  if (res !== "joined" && res !== "waiting") return res;
  const mine: JoinSent = { bootIdAtSend: typeof store.state?.boot_id === "string" ? store.state.boot_id : null, waiting: res === "waiting" };
  sent.value = mine;
  deps.setTimer(() => { if (sent.value === mine) sent.value = null; }, res === "joined" ? JOIN_PROMISE_MS : SENT_WAITING_MS);
  return res;
}

function titleText(hasNet: boolean, choice: JoinForm["choice"]): string {
  if (hasNet) return HOME_TITLE_SWITCH;
  return choice === "create" ? HOME_TITLE_CREATE : HOME_TITLE_JOIN;
}

// An invite in the address is read once, removed from the address and the history entry, and kept in memory for the
// join forms: the one that is mounted may be on another tab than the one that opens first.
let heldInvite = "";
let addressRead = false;

function takeInviteFromAddress(form: JoinForm): void {
  if (!addressRead && typeof location !== "undefined") {
    addressRead = true;
    heldInvite = inviteFromSearch(location.search);
    if (heldInvite) history.replaceState(null, "", withoutInvite(location.search, location.pathname, location.hash));
  }
  if (!heldInvite) return;
  form.inviteInput = heldInvite;
  form.choice = "join";
}

export function useJoin(store: Store, deps: HomeDeps = defaultHomeDeps): JoinView {
  const form = reactive(createJoinForm());
  takeInviteFromAddress(form);
  const home = inject(HOME_KEY, null);
  const sent = home ? home.joinSent : ref<JoinSent | null>(null);
  const hasNet = computed(() => !!(store.state && store.state.network));
  const err = computed(() => form.inviteError);
  const showJoin = computed(() => hasNet.value || form.choice === "join");
  const showCreate = computed(() => form.choice === "create");
  const title = computed(() => titleText(hasNet.value, form.choice));

  return {
    form, hasNet,
    showJoin, showCreate, title,
    buttonText: computed(() => (hasNet.value ? HOME_BUTTON_SWITCH : HOME_BUTTON_JOIN)),
    errorParts: computed(() => (err.value ? highlightParts(err.value.message, err.value.word) : [])),
    progress: computed(() => joinProgress(store.state, sent.value, store.absence ?? null)),
    join: () => joinNetwork(form, store, deps).then((res) => markSent(res, sent, store, deps)),
    create: () => createNetwork(form, store, deps),
    openJoin: () => { form.choice = "join"; },
    openCreate: () => { form.choice = "create"; },
    toggleCreate: () => { form.choice = form.choice === "create" ? "" : "create"; },
  };
}
