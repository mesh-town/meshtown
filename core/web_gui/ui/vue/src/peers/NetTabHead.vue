<script setup lang="ts">
// The header of a network tab: the name with the Managed badge, the line "You and 5 others · 4 online · Admins: …" and the invite button. The invite
// string is a network secret and stays in `useInvite`; the page store never sees it. It is shown in `InviteShareDialog`.
import { inject, ref } from "vue";
import { STORE_KEY } from "../core/store.ts";
import InviteNameDialog from "../home/InviteNameDialog.vue";
import InviteShareDialog from "../home/InviteShareDialog.vue";
import { useInvite } from "../home/use-invite.ts";
import { PEERS_INVITE_ONE, PEERS_MANAGED_BADGE, peersNetLine } from "./peers-texts.ts";

const props = defineProps<{ slot: string; name: string; who: string; admins: string; note?: string | null }>();
const store = inject(STORE_KEY)!;
const { inviteText, inviteQr, busy, status, warning, request } = useInvite(store, undefined, () => props.slot);
const nameDialog = ref<InstanceType<typeof InviteNameDialog>>();
const shareDialog = ref<InstanceType<typeof InviteShareDialog>>();
const shareName = ref("");

async function invite(name: string): Promise<void> {
  shareName.value = name;
  if ((await request(name)) === "ready") shareDialog.value?.open();
}
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
  <InviteShareDialog ref="shareDialog" :text="inviteText" :qr="inviteQr" :name="shareName" :warning="warning" />
  <p v-if="status" class="hint" id="peers-invite-status" role="status">{{ status }}</p>
</template>
