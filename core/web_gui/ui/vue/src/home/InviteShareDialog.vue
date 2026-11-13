<script setup lang="ts">
// The window after "Create invite": how to hand the invite to the new member — as text or as a QR code. The invite string is a
// network secret: it arrives as a prop from `useInvite` and is not kept here; closing the window forgets the copy state.
import { ref, useId, watch } from "vue";
import { writeClipboard } from "../common/clipboard.ts";
import InviteShareIcon from "./InviteShareIcon.vue";
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
const emit = defineEmits<{ closed: [] }>();
const mode = ref<"text" | "qr">("text");
const copy = ref<"" | "done" | "manual">("");

function open(start: "text" | "qr" = "text"): void {
  mode.value = start;
  copy.value = "";
  if (dialog.value && !dialog.value.open) dialog.value.showModal();
}

function close(): void {
  copy.value = "";
  dialog.value?.close();
}

function onClosed(): void {
  copy.value = "";
  emit("closed");
}

async function copyText(): Promise<void> {
  const done = await writeClipboard(defaultHomeDeps, props.text);
  copy.value = done ? "done" : "manual";
}

watch(() => props.text, (text) => !text && close());
defineExpose({ open, close });
</script>

<template>
  <dialog ref="dialog" class="member-dialog invite-share-dialog" :aria-labelledby="id + '-title'" @close="onClosed">
    <h2 :id="id + '-title'" class="member-dialog-title">{{ inviteShareTitle(name) }}</h2>
    <p class="hint">{{ INVITE_SHARE_LEAD }}</p>
    <ol class="invite-share-steps">
      <li><a :href="UPDATE_DOWNLOAD_URL" target="_blank" rel="noopener noreferrer">{{ INVITE_SHARE_INSTALL }}</a></li>
      <li>{{ INVITE_SHARE_OPEN }}</li>
      <li>{{ mode === "qr" ? INVITE_SHARE_SCAN : INVITE_SHARE_PASTE }}</li>
    </ol>
    <div class="invite-share-modes" role="group" :aria-label="INVITE_SHARE_GROUP">
      <button type="button" class="btn" :class="{ 'btn-primary': mode === 'text' }" id="invite-share-text-btn" :aria-pressed="mode === 'text'" @click="mode = 'text'">
        <InviteShareIcon kind="copy" />
        {{ INVITE_SHARE_AS_TEXT }}
      </button>
      <button type="button" class="btn" :class="{ 'btn-primary': mode === 'qr' }" id="invite-share-qr-btn" :aria-pressed="mode === 'qr'" @click="mode = 'qr'">
        <InviteShareIcon kind="qr" />
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
