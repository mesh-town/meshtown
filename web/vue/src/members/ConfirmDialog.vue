<script setup lang="ts">
// The one confirmation dialog of the Members tab (remove, confirm, block, leave): a native modal `<dialog>`.
// `ask(spec)` resolves true only for the action button; Cancel, Esc, a click on the backdrop or the removal of the
// component answer false. Does not call the API: the caller acts on the answer. Cancel takes the focus on open.
// A spec with `field` also draws a one-line reason input; `reason()` gives its cleaned text for the last opening.
import { nextTick, onUnmounted, ref } from "vue";
import type { DialogSpec } from "./dialog-specs.ts";
import { cleanReason } from "./member-reason.ts";
import { DIALOG_CANCEL } from "./members-texts.ts";

const dlg = ref<HTMLDialogElement | null>(null);
const cancelBtn = ref<HTMLButtonElement | null>(null);
const spec = ref<DialogSpec | null>(null);
const draft = ref("");
let resolver: ((answer: boolean) => void) | null = null;
let opener: HTMLElement | null = null;

async function open(el: HTMLDialogElement, next: DialogSpec, resolve: (answer: boolean) => void): Promise<void> {
  resolver = resolve;
  opener = document.activeElement as HTMLElement | null;
  spec.value = next;
  draft.value = "";
  await nextTick();
  el.returnValue = ""; // Esc leaves returnValue alone: without a reset an earlier "yes" would surface.
  el.showModal();
  cancelBtn.value?.focus();
}

function ask(next: DialogSpec): Promise<boolean> {
  const el = dlg.value;
  if (!el || typeof el.showModal !== "function") return Promise.resolve(false);
  return new Promise<boolean>((resolve) => void open(el, next, resolve));
}

function onClose(): void {
  const resolve = resolver;
  const back = opener;
  resolver = null;
  opener = null;
  if (resolve) resolve(dlg.value?.returnValue === "yes");
  if (back && back.isConnected) back.focus();
}

const finish = (answer: "yes" | "no"): void => dlg.value?.close(answer);
const onBackdrop = (ev: MouseEvent): void => {
  if (ev.target === dlg.value) finish("no");
};

// Tab goes round inside the dialog.
function onKey(ev: KeyboardEvent): void {
  if (ev.key !== "Tab" || !dlg.value) return;
  const buttons = Array.from(dlg.value.querySelectorAll<HTMLElement>("input, button"));
  const edge = ev.shiftKey ? buttons[0] : buttons[buttons.length - 1];
  const target = ev.shiftKey ? buttons[buttons.length - 1] : buttons[0];
  if (document.activeElement !== edge) return;
  ev.preventDefault();
  target.focus();
}

onUnmounted(() => resolver && resolver(false));

const reason = (): string => (spec.value && spec.value.field ? cleanReason(draft.value) : "");

defineExpose({ ask, reason });
</script>

<template>
  <dialog ref="dlg" class="member-dialog" role="alertdialog" aria-labelledby="confirm-title" aria-describedby="confirm-body" @close="onClose" @click="onBackdrop" @keydown="onKey">
    <template v-if="spec">
      <h2 id="confirm-title" class="member-dialog-title">{{ spec.title }}</h2>
      <div id="confirm-body">
        <p v-for="(line, i) in spec.body" :key="i">{{ line }}</p>
        <p v-if="spec.hint" class="hint">{{ spec.hint }}</p>
      </div>
      <template v-if="spec.field">
        <label for="member-reason">{{ spec.field.label }}</label>
        <input id="member-reason" v-model="draft" type="text" class="input" :maxlength="spec.field.max" autocomplete="off" @keydown.enter.prevent />
      </template>
      <div class="ports-keep-actions">
        <button id="member-confirm-yes" type="button" class="btn" :class="{ 'btn-danger': spec.danger }" @click="finish('yes')">{{ spec.action }}</button>
        <button id="member-confirm-cancel" ref="cancelBtn" type="button" class="btn btn-primary" autofocus @click="finish('no')">{{ DIALOG_CANCEL }}</button>
      </div>
    </template>
  </dialog>
</template>
