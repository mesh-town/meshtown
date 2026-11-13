<script setup lang="ts">
// The header of a network tab: the name with the Managed badge, the line "You and 5 others · 4 online · Admins: …" and the invite button. The invite
// string is a network secret and stays in `useInvite`; the page store never sees it.
import { inject, onUnmounted, ref } from "vue";
import { STORE_KEY } from "../core/store.ts";
import InviteNameDialog from "../home/InviteNameDialog.vue";
import { useInvite } from "../home/use-invite.ts";
import { PEERS_INVITE_COPIED, PEERS_INVITE_ONE, PEERS_MANAGED_BADGE, peersNetLine } from "./peers-texts.ts";

const COPIED_MS = 10_000;
const props = defineProps<{ slot: string; name: string; who: string; admins: string; note?: string | null }>();
const store = inject(STORE_KEY)!;
const { inviteText, busy, status, warning, request } = useInvite(store, undefined, () => props.slot);
const nameDialog = ref<InstanceType<typeof InviteNameDialog>>();
const copied = ref(false);
const manual = ref(false);
const failed = ref(false);
let timer: ReturnType<typeof setTimeout> | null = null;

async function invite(name: string): Promise<void> {
  const result = await request(name);
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
    <div class="tabhead-main">
      <h3 class="tabhead-name">{{ name }}<span class="badge net-badge">{{ PEERS_MANAGED_BADGE }}</span></h3>
      <p class="tabhead-line" id="peers-net-line">{{ peersNetLine(who) }}<template v-if="admins"> · <span id="admins-line">{{ admins }}</span></template></p>
    </div>
    <p v-if="note" class="hint" id="peers-invite-note">{{ note }}</p>
    <button v-else type="button" class="btn btn-primary" id="peers-invite-btn" :disabled="busy" @click="nameDialog?.open()">{{ PEERS_INVITE_ONE }}</button>
  </div>
  <InviteNameDialog v-if="!note" ref="nameDialog" :slot="slot" @confirm="invite" />
  <p v-if="copied" class="hint" id="peers-invite-status" role="status">{{ PEERS_INVITE_COPIED }}</p>
  <p v-else-if="manual || failed" class="hint" id="peers-invite-status" role="status">{{ status }}</p>
  <p v-if="manual" class="hint" id="peers-invite-value-wrap"><code class="mono invite-value" id="peers-invite-value">{{ inviteText }}</code></p>
  <p v-if="(copied || manual) && warning" class="hint hint-warn" id="peers-invite-warning">{{ warning }}</p>
</template>
