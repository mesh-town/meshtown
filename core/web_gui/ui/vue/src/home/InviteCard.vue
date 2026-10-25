<script setup lang="ts">
// "Invite a member": the button, the member counter, the network status. The invite string is a secret,
// lives in `inviteText` (`use-invite.ts`); `full` — a separate full-width card.
import { inject } from "vue";
import { STORE_KEY } from "../core/store.ts";
import { HOME_INVITE_BTN, HOME_INVITE_FOR_LABEL, HOME_INVITE_HINT, HOME_INVITE_TITLE, HOME_INVITE_VALUE_ARIA } from "./home-card-texts.ts";
import { HOME_INVITE_QR_ALT, HOME_INVITE_QR_TOO_LONG } from "./invite-qr-texts.ts";
import { useHomeContext } from "./use-home.ts";
import { useInvite } from "./use-invite.ts";
import { useInviteNote } from "../peers/invite-note.ts";

defineProps<{ full?: boolean }>();
const store = inject(STORE_KEY)!;
const { hasNet, members, netStatus, notice } = useHomeContext(store);
const { inviteText, inviteQr, busy, status, warning, forWhom, request } = useInvite(store);
const note = useInviteNote(store, () => store.lead);
</script>

<template>
  <div v-if="hasNet" class="card" :class="{ 'card-full': full }" id="home-invite" role="region" aria-labelledby="home-invite-title">
    <h2 id="home-invite-title">{{ HOME_INVITE_TITLE }}</h2>
    <p v-if="note" class="hint" id="peers-invite-note">{{ note }}</p>
    <template v-else>
      <label for="invite-for-input" class="hint">{{ HOME_INVITE_FOR_LABEL }}</label>
      <input id="invite-for-input" v-model="forWhom" type="text" class="input input-wide" maxlength="64" autocomplete="off" @keydown.enter.prevent="request" />
      <div class="btn-row"><button type="button" class="btn btn-primary" id="netkey-invite-btn" :disabled="busy" @click="request">{{ HOME_INVITE_BTN }}</button></div>
    </template>
    <p class="hint" id="home-members-count" :hidden="!members">{{ members }}</p>
    <p class="hint" id="home-net-status" :hidden="!netStatus">{{ netStatus }}</p>
    <p v-if="!note" class="hint" id="invite-hint">{{ HOME_INVITE_HINT }}</p>
    <p class="hint" id="invite-status" role="status" :hidden="!(status || notice)">{{ status || notice }}</p>
    <p class="hint" id="invite-value-wrap" :hidden="!inviteText"><code class="mono invite-value" id="invite-value" :aria-label="HOME_INVITE_VALUE_ARIA">{{ inviteText }}</code></p>
    <p class="hint hint-warn" id="invite-reach-warning" :hidden="!warning">{{ warning }}</p>
    <img v-if="inviteText && inviteQr" id="invite-qr" class="invite-qr" :src="inviteQr" :alt="HOME_INVITE_QR_ALT">
    <p v-else-if="inviteText" id="invite-qr-too-long" class="hint">{{ HOME_INVITE_QR_TOO_LONG }}</p>
  </div>
</template>
