<script setup lang="ts">
// "Invite a member": the button, the member counter, the network status; the invite itself is shown in `InviteShareDialog`. The invite string is a secret,
// lives in `inviteText` (`use-invite.ts`); `full` — a separate full-width card.
import { inject, ref } from "vue";
import { STORE_KEY } from "../core/store.ts";
import { HOME_INVITE_BTN, HOME_INVITE_HINT, HOME_INVITE_TITLE } from "./home-card-texts.ts";
import { useHomeContext } from "./use-home.ts";
import InviteNameDialog from "./InviteNameDialog.vue";
import InviteShareDialog from "./InviteShareDialog.vue";
import { useInvite } from "./use-invite.ts";
import { useInviteNote } from "../peers/invite-note.ts";

defineProps<{ full?: boolean; quiet?: boolean }>();
const store = inject(STORE_KEY)!;
const { hasNet, members, netStatus, notice } = useHomeContext(store);
const { inviteText, inviteQr, busy, status, warning, request } = useInvite(store);
const nameDialog = ref<InstanceType<typeof InviteNameDialog>>();
const shareDialog = ref<InstanceType<typeof InviteShareDialog>>();
const shareName = ref("");
const note = useInviteNote(store, () => store.lead);

async function invite(name: string): Promise<void> {
  shareName.value = name;
  if ((await request(name)) === "ready") shareDialog.value?.open();
}
</script>

<template>
  <div v-if="hasNet" class="card" :class="{ 'card-full': full }" id="home-invite" role="region" aria-labelledby="home-invite-title">
    <h2 id="home-invite-title">{{ HOME_INVITE_TITLE }}</h2>
    <p v-if="note" class="hint" id="peers-invite-note">{{ note }}</p>
    <template v-else>
      <div class="btn-row"><button type="button" :class="quiet ? 'btn' : 'btn btn-primary'" id="netkey-invite-btn" :disabled="busy" @click="nameDialog?.open()">{{ HOME_INVITE_BTN }}</button></div>
      <InviteNameDialog ref="nameDialog" :slot="store.lead" @confirm="invite" />
    </template>
    <InviteShareDialog ref="shareDialog" :text="inviteText" :qr="inviteQr" :name="shareName" :warning="warning" />
    <p class="hint" id="home-members-count" :hidden="!members">{{ members }}</p>
    <p class="hint" id="home-net-status" :hidden="!netStatus">{{ netStatus }}</p>
    <p v-if="!note" class="hint" id="invite-hint">{{ HOME_INVITE_HINT }}</p>
    <p class="hint" id="invite-status" role="status" :hidden="!(status || notice)">{{ status || notice }}</p>
  </div>
</template>
