<script setup lang="ts">
// "Which networks can connect": network rows with checkboxes, Save / Cancel, return to automatic choice.
import { inject } from "vue";
import { STORE_KEY } from "../core/store.ts";
import { BTN_CANCEL, BTN_SAVE } from "./nets-labels.ts";
import { AUTO_BTN, AUTO_NOTE, EMPTY_HINT, HERE_NOTE, NETS_NO_STORE, NETS_SUB, NETS_TAG, NETS_TITLE, WARN_PROVIDER } from "./nets-texts.ts";
import NetsSaved from "./NetsSaved.vue";
import { useNets } from "./use-nets.ts";
import type { NetRow } from "./use-nets.ts";

const props = defineProps<{ context: "settings" | "wizard"; hideSaved?: boolean }>();
const emit = defineEmits<{ (e: "saved"): void }>();
const store = inject(STORE_KEY);
if (!store) throw new Error(NETS_NO_STORE);
const n = useNets(store);

function onClick(row: NetRow, ev: Event): void {
  if (row.locked) n.lockedClick(ev);
}
function onChange(row: NetRow, ev: Event): void {
  if (!row.locked) n.toggle(row.name, (ev.target as HTMLInputElement).checked);
}
async function done(ok: boolean): Promise<void> {
  if (ok) emit("saved");
}
</script>

<template>
  <div id="nets-block" class="nets">
    <div class="nets-head"><b id="nets-title" class="nets-title">{{ NETS_TITLE }}</b><span id="nets-tag" class="mini-badge mini-badge-restart">{{ NETS_TAG }}</span></div>
    <div id="nets-sub" class="nets-sub">{{ NETS_SUB }}</div>
    <template v-for="r in n.rows.value" :key="r.name">
      <label class="net-row" :class="{ locked: r.locked }">
        <input type="checkbox" :id="'net-' + r.name" :checked="r.checked" :aria-disabled="r.locked ? 'true' : undefined" @click="onClick(r, $event)" @change="onChange(r, $event)">
        <span class="net-main"><span class="net-name">{{ r.title }}</span><span class="net-note"><template v-if="r.hint">{{ r.hint }} · </template><code>{{ r.code }}</code></span></span>
        <span v-if="r.badge" class="net-badge">{{ r.badge }}</span>
      </label>
      <div v-if="r.locked && n.hereNote.value" id="nets-here-note" class="net-here-note">{{ HERE_NOTE }}</div>
      <div v-if="r.warn" :id="'net-warn-' + r.name" class="net-warn">{{ WARN_PROVIDER }}</div>
    </template>
    <div class="nets-foot">
      <button v-if="n.hasSavedNetList.value" type="button" id="nets-auto-btn" class="nets-link" :disabled="n.busy.value" @click="n.setAuto().then(done)">{{ AUTO_BTN }}</button>
      <span v-else id="nets-auto-note">{{ AUTO_NOTE }}</span>
    </div>
    <div class="nets-btns">
      <button type="button" id="nets-save" class="btn btn-primary" :disabled="!n.canSave.value" @click="n.save().then(done)">{{ BTN_SAVE }}</button>
      <button type="button" id="nets-cancel" class="btn" :disabled="!n.dirty.value" @click="n.cancel">{{ BTN_CANCEL }}</button>
      <span v-if="n.empty.value" id="nets-empty-hint" class="hint">{{ EMPTY_HINT }}</span>
    </div>
    <p id="nets-error" class="field-error-text" :hidden="!n.error.value">{{ n.error.value }}</p>
    <NetsSaved v-if="!props.hideSaved && n.showSaved.value" :phase="n.phase.value" :node-is-router="n.nodeIsRouter.value" :show-restart="props.context === 'wizard'" @restart="n.restart" />
  </div>
</template>
