<script setup lang="ts">
// The "Add another network" form: join with an invite or create a network, in a new slot. One component for two places —
// under the Members list (`peers`) and in Settings → Network (`settings`); only "Cancel" differs. On success it moves to
// the new network's tab and leaves a banner there; a join that waits for a member shows its progress in place.
import { computed, inject, reactive, ref } from "vue";
import { STORE_KEY } from "../core/store.ts";
import { netName, visibleSlots } from "../core/slots.ts";
import { peersHash } from "../shell/nav.ts";
import { defaultHomeDeps } from "./home-actions.ts";
import { createJoinForm, highlightParts } from "./join-actions.ts";
import { addNetwork, addedBannerText, atLimit, stayText } from "./add-net-actions.ts";
import type { AddNetForm, AddNetMode, AddNetResult } from "./add-net-actions.ts";
import { useNetBanner } from "./add-net-banner.ts";
import {
  ADD_NET_BTN, ADD_NET_BUSY, ADD_NET_CANCEL, ADD_NET_CREATE_BTN, ADD_NET_CREATE_LABEL, ADD_NET_CREATE_PLACEHOLDER, ADD_NET_JOIN_HINT,
  ADD_NET_JOIN_LABEL, ADD_NET_JOIN_PLACEHOLDER, ADD_NET_LIMIT, ADD_NET_TITLE, ADD_NET_TO_CREATE, ADD_NET_TO_JOIN, ADD_NET_WAITING,
  ADD_NET_WAITING_SUB, duplicateText, openNetText,
} from "./add-net-texts.ts";

const props = defineProps<{ context: "peers" | "settings" }>();
const emit = defineEmits<{ done: [result: AddNetResult, slot: string | null] }>();
const store = inject(STORE_KEY)!;
const form = reactive<AddNetForm>(createJoinForm());
const mode = ref<AddNetMode>("join");
const outcome = ref<AddNetResult | "">("");
const { show } = useNetBanner();

const names = computed(() => visibleSlots(store.nets, store.slots).map((s) => netName(store.nets, s)));
const limited = computed(() => outcome.value === "limit" || atLimit(store));
const dupName = computed(() => (form.dupSlot ? netName(store.nets, form.dupSlot) : ""));
const errorParts = computed(() => (form.inviteError ? highlightParts(form.inviteError.message, form.inviteError.word) : []));
const creating = computed(() => mode.value === "create");

function arrive(result: AddNetResult): void {
  const slot = form.slot ?? null;
  const text = addedBannerText(store, form, result);
  if (slot && text) {
    show(slot, text);
    location.hash = peersHash(slot);
  }
}

async function submit(): Promise<void> {
  const result = await addNetwork(form, store, defaultHomeDeps, mode.value);
  outcome.value = result;
  if (result === "joined" || result === "created") arrive(result);
  emit("done", result, result === "joined" || result === "created" || result === "waiting" ? form.slot ?? null : null);
}

function toggle(): void {
  mode.value = creating.value ? "join" : "create";
  form.inviteError = null;
  form.labelError = null;
  outcome.value = "";
}

function cancel(): void {
  if (props.context === "peers") {
    location.hash = peersHash(store.lead);
    return;
  }
  Object.assign(form, createJoinForm());
  mode.value = "join";
  outcome.value = "";
}

const openDup = (): void => {
  if (form.dupSlot) location.hash = peersHash(form.dupSlot);
};
</script>

<template>
  <div class="card" id="add-net-card" role="region" aria-labelledby="add-net-title" :data-context="context">
    <div class="step-head"><h2 id="add-net-title">{{ ADD_NET_TITLE }}</h2></div>
    <p v-if="limited" class="hint" id="add-net-limit" role="status">{{ ADD_NET_LIMIT }}</p>
    <template v-else>
      <p class="hint" id="add-net-stay">{{ stayText(names) }}</p>
      <div v-if="!creating" id="add-net-join-box">
        <label class="field-label" for="add-net-input">{{ ADD_NET_JOIN_LABEL }}</label>
        <input type="text" class="input mono input-wide" :class="{ 'field-error': form.inviteError }" id="add-net-input" v-model="form.inviteInput" autocomplete="off" spellcheck="false" aria-describedby="add-net-error" :placeholder="ADD_NET_JOIN_PLACEHOLDER">
        <p class="field-error-text" id="add-net-error" role="alert" :hidden="!form.inviteError"><template v-for="(part, i) in errorParts" :key="i"><mark v-if="part.highlighted" class="key-error-word">{{ part.text }}</mark><template v-else>{{ part.text }}</template></template></p>
        <p class="hint">{{ ADD_NET_JOIN_HINT }}</p>
      </div>
      <div v-else id="add-net-create-box">
        <label class="field-label" for="add-net-label-input">{{ ADD_NET_CREATE_LABEL }}</label>
        <input type="text" class="input" :class="{ 'field-error': form.labelError }" id="add-net-label-input" v-model="form.label" :placeholder="ADD_NET_CREATE_PLACEHOLDER">
        <p class="field-error-text" id="add-net-label-error" role="alert" :hidden="!form.labelError">{{ form.labelError }}</p>
      </div>
      <div class="btn-row">
        <button type="button" class="btn btn-primary" id="add-net-btn" :disabled="!!form.busy" @click="submit">{{ form.busy ? ADD_NET_BUSY : creating ? ADD_NET_CREATE_BTN : ADD_NET_BTN }}</button>
        <button type="button" class="btn" id="add-net-cancel" @click="cancel">{{ ADD_NET_CANCEL }}</button>
      </div>
      <a href="#" class="link-more" id="add-net-toggle" @click.prevent="toggle">{{ creating ? ADD_NET_TO_JOIN : ADD_NET_TO_CREATE }}</a>
      <div v-if="outcome === 'waiting'" id="add-net-waiting" role="status">
        <p class="hint">{{ ADD_NET_WAITING }}</p>
        <p class="hint">{{ ADD_NET_WAITING_SUB }}</p>
      </div>
      <div v-if="outcome === 'duplicate'" id="add-net-dup" role="status">
        <p class="hint">{{ duplicateText(dupName) }}</p>
        <button type="button" class="btn" id="add-net-open" @click="openDup">{{ openNetText(dupName) }}</button>
      </div>
    </template>
  </div>
</template>
