<script setup lang="ts">
// The window after "Create invite": how to hand the invite to the new member — as text or as a QR code. The invite string is a
// network secret: it arrives as a prop from `useInvite` and is not kept here; closing the window forgets the copy state.
import { ref, useId, watch } from "vue";
import { writeClipboard } from "../common/clipboard.ts";
import { defaultHomeDeps } from "./home-actions.ts";
import { HOME_INVITE_VALUE_ARIA } from "./home-card-texts.ts";
import { HOME_INVITE_QR_ALT, HOME_INVITE_QR_TOO_LONG } from "./invite-qr-texts.ts";
import {
  INVITE_SHARE_AS_QR, INVITE_SHARE_AS_TEXT, INVITE_SHARE_CLOSE, INVITE_SHARE_COPIED, INVITE_SHARE_COPY, INVITE_SHARE_GROUP, INVITE_SHARE_INSTALL,
  INVITE_SHARE_LEAD, INVITE_SHARE_MANUAL, INVITE_SHARE_NOTE, INVITE_SHARE_OPEN, INVITE_SHARE_PASTE, INVITE_SHARE_SCAN, inviteShareTitle,
} from "./invite-share-texts.ts";
import { UPDATE_DOWNLOAD_URL } from "./update-texts.ts";

const props = defineProps<{ text: string; qr: string; name: string; warning: string }>();
const id = useId();
const dialog = ref<HTMLDialogElement>();
const mode = ref<"text" | "qr">("text");
const copy = ref<"" | "done" | "manual">("");

function open(): void {
  mode.value = "text";
  copy.value = "";
  if (dialog.value && !dialog.value.open) dialog.value.showModal();
}

function close(): void {
  copy.value = "";
  dialog.value?.close();
}

async function copyText(): Promise<void> {
  const done = await writeClipboard(defaultHomeDeps, props.text);
  copy.value = done ? "done" : "manual";
}

watch(() => props.text, (text) => !text && close());
defineExpose({ open, close });
</script>

<template>
  <dialog ref="dialog" class="member-dialog invite-share-dialog" :aria-labelledby="id + '-title'" @close="copy = ''">
    <h2 :id="id + '-title'" class="member-dialog-title">{{ inviteShareTitle(name) }}</h2>
    <p class="hint">{{ INVITE_SHARE_LEAD }}</p>
    <ol class="invite-share-steps">
      <li><a :href="UPDATE_DOWNLOAD_URL" target="_blank" rel="noopener noreferrer">{{ INVITE_SHARE_INSTALL }}</a></li>
      <li>{{ INVITE_SHARE_OPEN }}</li>
      <li>{{ mode === "qr" ? INVITE_SHARE_SCAN : INVITE_SHARE_PASTE }}</li>
    </ol>
    <div class="invite-share-modes" role="group" :aria-label="INVITE_SHARE_GROUP">
      <button type="button" class="btn" :class="{ 'btn-primary': mode === 'text' }" id="invite-share-text-btn" :aria-pressed="mode === 'text'" @click="mode = 'text'">
        <svg class="site-icon" width="16" height="16" viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <rect x="6" y="6" width="9" height="9" rx="1.5" /><path d="M12 3.5V3a1 1 0 0 0-1-1H3a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h.5" />
        </svg>
        {{ INVITE_SHARE_AS_TEXT }}
      </button>
      <button type="button" class="btn" :class="{ 'btn-primary': mode === 'qr' }" id="invite-share-qr-btn" :aria-pressed="mode === 'qr'" @click="mode = 'qr'">
        <svg class="site-icon" width="16" height="16" viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <rect x="2" y="2" width="5" height="5" /><rect x="11" y="2" width="5" height="5" /><rect x="2" y="11" width="5" height="5" /><path d="M11 11h2v2h-2zM15 15h1M11 15h1" />
        </svg>
        {{ INVITE_SHARE_AS_QR }}
      </button>
    </div>
    <div v-if="mode === 'text'" class="invite-share-body" id="invite-share-text">
      <p class="hint"><code class="mono invite-value" id="invite-value" :aria-label="HOME_INVITE_VALUE_ARIA">{{ text }}</code></p>
      <div class="btn-row">
        <button type="button" class="btn" id="invite-share-copy" @click="copyText">{{ INVITE_SHARE_COPY }}</button>
        <span v-if="copy" class="hint" id="invite-share-copy-status" role="status">{{ copy === "done" ? INVITE_SHARE_COPIED : INVITE_SHARE_MANUAL }}</span>
      </div>
    </div>
    <div v-else class="invite-share-body" id="invite-share-qr">
      <img v-if="qr" id="invite-qr" class="invite-qr" :src="qr" :alt="HOME_INVITE_QR_ALT">
      <p v-else-if="text" id="invite-qr-too-long" class="hint">{{ HOME_INVITE_QR_TOO_LONG }}</p>
    </div>
    <p class="hint">{{ INVITE_SHARE_NOTE }}</p>
    <p class="hint hint-warn" id="invite-reach-warning" :hidden="!warning">{{ warning }}</p>
    <div class="dialog-actions">
      <button type="button" class="btn btn-primary" id="invite-share-close" @click="close">{{ INVITE_SHARE_CLOSE }}</button>
    </div>
  </dialog>
</template>
