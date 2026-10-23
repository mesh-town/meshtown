<script setup lang="ts">
// The header of a network tab: the invite button and, with several networks, the network line. The invite
// string is a network secret and stays in `useInvite`; the page store never sees it.
import { inject, onUnmounted, ref } from "vue";
import { STORE_KEY } from "../core/store.ts";
import { useInvite } from "../home/use-invite.ts";
import { PEERS_INVITE_COPIED, PEERS_INVITE_ONE, peersInviteTo, peersNetLine } from "./peers-texts.ts";

const COPIED_MS = 10_000;
const props = defineProps<{ slot: string; name: string; who: string; multi: boolean }>();
const store = inject(STORE_KEY)!;
const { inviteText, busy, status, warning, request } = useInvite(store, undefined, () => props.slot);
const copied = ref(false);
const manual = ref(false);
const failed = ref(false);
let timer: ReturnType<typeof setTimeout> | null = null;

async function invite(): Promise<void> {
  const result = await request();
  if (result === "stale" || result === "skipped") return;
  copied.value = result === "copied";
  manual.value = result === "manual";
  failed.value = result === "failed";
  if (timer) clearTimeout(timer);
  timer = result === "copied" ? setTimeout(() => (copied.value = false), COPIED_MS) : null;
}
onUnmounted(() => timer && clearTimeout(timer));
</script>

<template>
  <div class="tabhead" id="peers-tabhead">
    <button type="button" class="btn btn-primary" id="peers-invite-btn" :disabled="busy" @click="invite">{{ multi ? peersInviteTo(name) : PEERS_INVITE_ONE }}</button>
    <span v-if="multi" class="quiet" id="peers-net-line">{{ peersNetLine(name, who) }}</span>
  </div>
  <p v-if="copied" class="hint" id="peers-invite-status" role="status">{{ PEERS_INVITE_COPIED }}</p>
  <p v-else-if="manual || failed" class="hint" id="peers-invite-status" role="status">{{ status }}</p>
  <p v-if="manual" class="hint" id="peers-invite-value-wrap"><code class="mono invite-value" id="peers-invite-value">{{ inviteText }}</code></p>
  <p v-if="(copied || manual) && warning" class="hint hint-warn" id="peers-invite-warning">{{ warning }}</p>
</template>
