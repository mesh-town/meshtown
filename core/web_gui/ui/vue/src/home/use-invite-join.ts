/* The composable of the one "Join with an invite" form: the form, the preview and the note under the field, the submit.
 * The same form sits on Home, in "Add another network" and under the card of a network the computer left. */

import { computed, nextTick, reactive, ref } from "vue";
import type { Ref } from "vue";
import type { Store } from "../core/store.ts";
import { peersHash } from "../shell/nav.ts";
import { defaultHomeDeps } from "./home-actions.ts";
import type { HomeDeps } from "./home-actions.ts";
import { addedBannerText, atLimit } from "./add-net-actions.ts";
import { expectArrival, useNetBanner } from "./add-net-banner.ts";
import { ADD_NET_LIMIT, openNetText } from "./add-net-texts.ts";
import { useInspectPreview } from "./inspect-preview.ts";
import { inviteFromSearch, withoutInvite } from "./invite-param.ts";
import { isInvite, netTitle, placeNote, sendInvite } from "./invite-join-actions.ts";
import type { InviteContext, InviteFormState, InviteOutcome } from "./invite-join-actions.ts";
import { afterMainJoin, createJoinForm, markFor } from "./join-actions.ts";

export type { InviteContext };

/** The ids of the form's parts; the places keep their old ones (`join-key-input`, `add-net-input`, `rejoin-input`, …). */
export interface InviteFormIds {
  input: string;
  button: string;
  error: string;
  preview: string;
  note: string;
  dup: string;
  open: string;
  status?: string;
  title?: string;
}

export interface InviteJoin {
  form: InviteFormState;
  input: Ref<HTMLInputElement | null>;
  preview: Readonly<Ref<string>>;
  note: Readonly<Ref<string>>;
  /** The "Open" link of an "already in" note. */
  noteLink: Readonly<Ref<{ slot: string; text: string } | null>>;
  outcome: Ref<InviteOutcome | "">;
  submit(): Promise<InviteOutcome>;
  focus(): void;
}

// An invite in the address is read once, removed from the address and the history entry, and kept in memory for the
// forms: the one that is mounted may be on another tab than the one that opens first.
let heldInvite = "";
let addressRead = false;

/** The invite the page was opened with (`?invite=`), empty when none or after it was used. The address is read once. */
export function addressInvite(): string {
  if (!addressRead && typeof location !== "undefined") {
    addressRead = true;
    heldInvite = inviteFromSearch(location.search);
    if (heldInvite) history.replaceState(null, "", withoutInvite(location.search, location.pathname, location.hash));
  }
  return heldInvite;
}

const spent = (outcome: InviteOutcome): boolean => outcome === "joined" || outcome === "waiting" || outcome === "here";

/** What a finished join does on another slot: a waiting one is watched for, a finished one gets its banner and its tab. */
export function useArrival(store: Store): (outcome: InviteOutcome, form: InviteFormState) => void {
  const { show } = useNetBanner();
  return (outcome, form) => {
    if (outcome === "waiting") expectArrival(form.slot ?? null);
    if (outcome !== "joined" || !form.slot) return;
    show(form.slot, addedBannerText(store, form, "joined") ?? "");
    location.hash = peersHash(form.slot);
  };
}

export function useInviteJoin(store: Store, ctx: InviteContext, deps: HomeDeps = defaultHomeDeps): InviteJoin {
  const form = reactive(createJoinForm());
  const held = addressInvite();
  if (held) {
    form.inviteInput = held;
    form.choice = "join";
  }
  const input = ref<HTMLInputElement | null>(null);
  const outcome = ref<InviteOutcome | "">("");
  const { preview, reply } = useInspectPreview(() => form.inviteInput);
  const note = computed(() => (reply.value ? placeNote(reply.value, store.nets, atLimit(store)) : ""));
  const noteLink = computed(() => {
    const slot = reply.value?.place === "here" ? reply.value.slot : undefined;
    return slot ? { slot, text: openNetText(netTitle(store.nets, slot, reply.value?.label)) } : null;
  });

  // Home: the mark of the first setup is decided before the request, while the node still has no network.
  async function submit(): Promise<InviteOutcome> {
    const mark = ctx.kind === "home" ? markFor(store) : null;
    const res = await sendOrRefuse();
    outcome.value = ctx.kind === "home" ? afterMainJoin(form, store, deps, res, mark) : res;
    if (spent(res)) heldInvite = "";
    return res;
  }

  // A word key needs a slot of its own in "Add": at the limit it is refused before any request. An invite goes anyway.
  function sendOrRefuse(): Promise<InviteOutcome> {
    const key = form.inviteInput.trim();
    if (ctx.kind !== "add" || !key || isInvite(key) || !atLimit(store)) return sendInvite(form, ctx, deps);
    form.inviteError = { message: ADD_NET_LIMIT, word: null };
    return Promise.resolve("limit");
  }

  return { form, input, preview, note, noteLink, outcome, submit, focus: () => void nextTick(() => input.value?.focus()) };
}
