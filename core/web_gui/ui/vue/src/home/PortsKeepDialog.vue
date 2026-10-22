<script setup lang="ts">
// The question when changing the network: "Keep publishing this computer's ports in the new network?".
// `ask()` → "yes" | "no" | "cancel"; no `showModal` — "no" (the safe answer).
import { onUnmounted, ref } from "vue";
import type { KeepAnswer } from "./home-actions.ts";
import { HOME_PORTS_CANCEL, HOME_PORTS_KEEP_TITLE, HOME_PORTS_NO, HOME_PORTS_YES } from "./home-card-texts.ts";

const dlg = ref<HTMLDialogElement | null>(null);
const noBtn = ref<HTMLButtonElement | null>(null);
let resolver: ((answer: KeepAnswer) => void) | null = null;

function open(el: HTMLDialogElement, resolve: (answer: KeepAnswer) => void): void {
  resolver = resolve;
  el.returnValue = ""; // Escape doesn't change returnValue: without a reset the previous "Yes" would surface.
  el.showModal();
  noBtn.value?.focus();
}

function ask(): Promise<KeepAnswer> {
  const el = dlg.value;
  return el && typeof el.showModal === "function"
    ? new Promise<KeepAnswer>((resolve) => open(el, resolve))
    : Promise.resolve("no");
}

function onClose(): void {
  const resolve = resolver;
  resolver = null;
  if (resolve) resolve((dlg.value?.returnValue || "cancel") as KeepAnswer);
}

const pick = (value: KeepAnswer): void => dlg.value?.close(value);

// Removal from the screen while the question is open: without an answer `ask()` would hang and hold the form ("skipped").
onUnmounted(() => resolver && resolver("cancel"));

defineExpose({ ask });
</script>

<template>
  <dialog ref="dlg" id="ports-keep-dialog" class="ports-keep-dialog" aria-labelledby="ports-keep-title" @close="onClose">
    <p id="ports-keep-title">{{ HOME_PORTS_KEEP_TITLE }}</p>
    <div class="ports-keep-actions">
      <button class="btn" id="ports-keep-cancel" type="button" value="cancel" @click="pick('cancel')">{{ HOME_PORTS_CANCEL }}</button>
      <button class="btn" id="ports-keep-yes" type="button" value="yes" @click="pick('yes')">{{ HOME_PORTS_YES }}</button>
      <button ref="noBtn" class="btn btn-primary" id="ports-keep-no" type="button" value="no" autofocus @click="pick('no')">{{ HOME_PORTS_NO }}</button>
    </div>
  </dialog>
</template>
