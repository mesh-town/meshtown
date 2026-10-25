/* Wrapper of the Home invite: the invite string is a network secret, lives only in `inv` (the `ref`
 * of this file, not exposed), not in the store, the form or the storages. The template reads `inviteText` (a `computed`):
 * the `ref` itself without `.value` is not rendered (the `NetKeyCard` error). */

import { computed, onUnmounted, ref, watch } from "vue";
import type { ComputedRef, Ref } from "vue";
import { stateOf } from "../core/slots.ts";
import type { Store } from "../core/store.ts";
import { createInviteState, qrImageSrc, requestInvite, resetInviteIfNetChanged } from "./invite-actions.ts";
import type { InviteResult, InviteState } from "./invite-actions.ts";
import { defaultHomeDeps } from "./home-actions.ts";
import type { HomeDeps } from "./home-actions.ts";

export interface InviteView {
  inviteText: ComputedRef<string>;
  inviteQr: ComputedRef<string>; // data URI of the QR picture, "" when none
  busy: ComputedRef<boolean>;
  status: ComputedRef<string>;
  warning: ComputedRef<string>;
  forWhom: Ref<string>; // the optional "for whom" label sent with the request
  request: () => Promise<InviteResult>;
}

const netIdOf = (store: Store, slot: string): string | null => stateOf(store, slot)?.network?.id_hex || null;

export function useInvite(store: Store, deps: Pick<HomeDeps, "api" | "clipboard"> = defaultHomeDeps, slot: () => string = () => store.lead): InviteView {
  const inv = ref<InviteState>(createInviteState());
  const forWhom = ref("");

  watch(() => netIdOf(store, slot()), (netId) => resetInviteIfNetChanged(inv.value, netId), { immediate: true });
  onUnmounted(() => resetInviteIfNetChanged(inv.value, null));

  return {
    inviteText: computed(() => inv.value.text),
    inviteQr: computed(() => qrImageSrc(inv.value.qr)),
    busy: computed(() => inv.value.busy),
    status: computed(() => inv.value.status),
    warning: computed(() => inv.value.warning),
    forWhom,
    request: () => requestInvite(inv.value, store, deps, forWhom.value, slot()),
  };
}
