/* Wrapper of the join card: form, computed texts and actions. The logic is in `join-actions.ts`. */

import { computed, inject, ref } from "vue";
import type { ComputedRef, Ref } from "vue";
import type { Store } from "../core/store.ts";
import { defaultHomeDeps } from "./home-actions.ts";
import type { HomeDeps } from "./home-actions.ts";
import { createNetwork, highlightParts } from "./join-actions.ts";
import type { JoinForm, JoinResult, HighlightPart } from "./join-actions.ts";
import { JOIN_PROMISE_MS } from "./join-actions.ts";
import { useInviteJoin } from "./use-invite-join.ts";
import type { InviteJoin } from "./use-invite-join.ts";
import { joinProgress } from "./join-progress.ts";
import type { JoinProgress, JoinSent } from "./join-progress.ts";
import { HOME_KEY } from "./use-home.ts";
import type { HomeContext } from "./use-home.ts";
import { HOME_BUTTON_JOIN, HOME_TITLE_CREATE, HOME_TITLE_JOIN } from "./home-flow-texts.ts";
import { INVITE_FORM_TITLE } from "./invite-join-texts.ts";

export interface JoinView {
  form: JoinForm;
  invite: InviteJoin;
  hasNet: ComputedRef<boolean>;
  showJoin: ComputedRef<boolean>;
  showCreate: ComputedRef<boolean>;
  title: ComputedRef<string>;
  buttonText: ComputedRef<string>;
  errorParts: ComputedRef<HighlightPart[]>;
  progress: ComputedRef<JoinProgress | null>;
  /** The sent invite the progress waits on (shared with Home when it provides one). */
  sent: Ref<JoinSent | null>;
  join: () => Promise<JoinResult>;
  create: () => Promise<JoinResult>;
  openJoin: () => void;
  openCreate: () => void;
  toggleCreate: () => void;
}

// "Join sent" holds until the node answers with a new start; the deadline insures against an endless spinner
// (waiting for members takes longer: its progress is shown by the node itself via `invite_join`).
const SENT_WAITING_MS = 10 * 60 * 1000;

function markSent(form: JoinForm, res: JoinResult, sent: Ref<JoinSent | null>, store: Store, deps: HomeDeps): void {
  if (form.slot !== "main" || (res !== "joined" && res !== "waiting")) return;
  const mine: JoinSent = { bootIdAtSend: typeof store.state?.boot_id === "string" ? store.state.boot_id : null, waiting: res === "waiting" };
  sent.value = mine;
  deps.setTimer(() => { if (sent.value === mine) sent.value = null; }, res === "joined" ? JOIN_PROMISE_MS : SENT_WAITING_MS);
}

function titleText(hasNet: boolean, choice: JoinForm["choice"]): string {
  if (hasNet) return INVITE_FORM_TITLE;
  return choice === "create" ? HOME_TITLE_CREATE : HOME_TITLE_JOIN;
}

export function useJoin(store: Store, deps: HomeDeps = defaultHomeDeps): JoinView {
  const base = useInviteJoin(store, { kind: "home" }, deps);
  const form = base.form;
  const home = inject(HOME_KEY, null);
  const sent = home ? home.joinSent : ref<JoinSent | null>(null);
  // The form's own button goes through the same hold: "sent" is what the progress block waits on.
  const invite: InviteJoin = { ...base, submit: () => base.submit().then((res) => { markSent(form, res, sent, store, deps); return res; }) };
  const hasNet = computed(() => !!(store.state && store.state.network));
  const err = computed(() => form.inviteError);
  const showJoin = computed(() => hasNet.value || form.choice === "join");
  const showCreate = computed(() => form.choice === "create");
  const title = computed(() => titleText(hasNet.value, form.choice));

  return {
    form, invite, hasNet, sent,
    showJoin, showCreate, title,
    buttonText: computed(() => HOME_BUTTON_JOIN),
    errorParts: computed(() => (err.value ? highlightParts(err.value.message, err.value.word) : [])),
    progress: computed(() => joinProgress(store.state, sent.value, store.absence ?? null)),
    join: () => invite.submit(),
    create: () => createNetwork(form, store, deps),
    openJoin: () => { form.choice = "join"; },
    openCreate: () => { form.choice = "create"; },
    toggleCreate: () => { form.choice = form.choice === "create" ? "" : "create"; },
  };
}
