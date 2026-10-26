<script setup lang="ts">
// The "Join or create another network" card: two tabs, "Join" with an invite or "Create" a network, in a new slot. It
// replaces the member list on the "+" tab. On success it moves to the new network's tab and leaves a banner there;
// a join that waits for a member shows its progress in place.
import { computed, inject, ref, watch } from "vue";
import { STORE_KEY } from "../core/store.ts";
import { netName, visibleSlots } from "../core/slots.ts";
import JoiningCard from "./JoiningCard.vue";
import InviteForm from "./InviteForm.vue";
import NetKindPicker from "./NetKindPicker.vue";
import { peersHash } from "../shell/nav.ts";
import { defaultHomeDeps } from "./home-actions.ts";
import { createJoinForm } from "./join-actions.ts";
import { addNetwork, addedBannerText, atLimit, stayText } from "./add-net-actions.ts";
import type { AddNetMode, AddNetResult } from "./add-net-actions.ts";
import { useNetBanner } from "./add-net-banner.ts";
import type { InviteOutcome } from "./invite-join-actions.ts";
import { ADD_NET_LIMIT_INVITE } from "./invite-join-texts.ts";
import { useArrival, useInviteJoin } from "./use-invite-join.ts";
import type { InviteFormIds } from "./use-invite-join.ts";
import {
  ADD_NET_BTN, ADD_NET_BUSY, ADD_NET_CANCEL, ADD_NET_CREATE_BTN, ADD_NET_CREATE_LABEL, ADD_NET_CREATE_PLACEHOLDER,
  ADD_NET_TAB_CREATE, ADD_NET_TAB_JOIN, ADD_NET_TABS_LABEL, ADD_NET_TITLE,
} from "./add-net-texts.ts";

const emit = defineEmits<{ done: [result: AddNetResult, slot: string | null] }>();
const store = inject(STORE_KEY)!;
const invite = useInviteJoin(store, { kind: "add" });
const { form, outcome } = invite;
const mode = ref<AddNetMode>("join");
const { show } = useNetBanner();
const arrival = useArrival(store);
const ids: InviteFormIds = {
  input: "add-net-input", button: "add-net-btn", error: "add-net-error", preview: "add-net-kind-preview", note: "add-net-place-note", dup: "add-net-dup", open: "add-net-open",
};

const names = computed(() => visibleSlots(store.nets, store.slots).map((s) => netName(store.nets, s)));
const limited = computed(() => atLimit(store));
const creating = computed(() => mode.value === "create");

// Joined: the new network's tab with its banner; waiting: watched for. Created: the banner and the tab.
function joined(result: InviteOutcome, slot: string | null): void {
  arrival(result, form);
  emit("done", result === "joined" || result === "waiting" ? result : "failed", result === "joined" || result === "waiting" ? slot : null);
}

async function create(): Promise<void> {
  const result = await addNetwork(form, store, defaultHomeDeps, "create");
  const slot = form.slot ?? null;
  const text = addedBannerText(store, form, result);
  if (result === "created" && slot && text) {
    show(slot, text);
    location.hash = peersHash(slot);
  }
  emit("done", result, result === "created" ? slot : null);
}

function pickMode(next: AddNetMode): void {
  mode.value = next;
  form.inviteError = null;
  form.labelError = null;
  outcome.value = "";
}

function resetForm(): void {
  Object.assign(form, createJoinForm());
  mode.value = "join";
  outcome.value = "";
}

// The slot of a waiting join got its key: the network is in, the form starts over (its tab is the banner's place).
watch(
  () => store.nets.find((n) => n.id === form.slot)?.id_hex ?? null,
  (idHex) => {
    if (idHex !== null && outcome.value === "waiting") resetForm();
  },
);

function cancel(): void {
  location.hash = peersHash(store.lead);
}

// A waiting join was cancelled: the form starts over and stays open.
function cancelled(): void {
  resetForm();
  store.peersTab = "add";
}
</script>

<template>
  <div class="card" id="add-net-card" role="region" aria-labelledby="add-net-title">
    <div class="step-head"><h2 id="add-net-title">{{ ADD_NET_TITLE }}</h2></div>
    <div v-if="!limited" class="home-tabs" role="tablist" :aria-label="ADD_NET_TABS_LABEL">
      <button type="button" class="home-tab" :class="{ active: !creating }" id="add-net-tab-join" role="tab" :aria-selected="!creating ? 'true' : 'false'" @click="pickMode('join')">{{ ADD_NET_TAB_JOIN }}</button>
      <button type="button" class="home-tab" :class="{ active: creating }" id="add-net-tab-create" role="tab" :aria-selected="creating ? 'true' : 'false'" @click="pickMode('create')">{{ ADD_NET_TAB_CREATE }}</button>
    </div>
    <p v-if="limited" class="hint" id="add-net-limit" role="status">{{ ADD_NET_LIMIT_INVITE }}</p>
    <p v-if="!limited" class="hint" id="add-net-stay">{{ stayText(names) }}</p>
    <InviteForm v-if="!creating" :join="invite" :ids="ids" :button="ADD_NET_BTN" :busy="ADD_NET_BUSY" @done="joined" />
    <div v-else id="add-net-create-box">
      <label class="field-label" for="add-net-label-input">{{ ADD_NET_CREATE_LABEL }}</label>
      <input type="text" class="input" :class="{ 'field-error': form.labelError }" id="add-net-label-input" v-model="form.label" :placeholder="ADD_NET_CREATE_PLACEHOLDER">
      <p class="field-error-text" id="add-net-label-error" role="alert" :hidden="!form.labelError">{{ form.labelError }}</p>
      <NetKindPicker v-model="form.kind" :name="form.label" />
      <div class="btn-row">
        <button type="button" class="btn btn-primary" id="add-net-btn" :disabled="!!form.busy" @click="create">{{ form.busy ? ADD_NET_BUSY : ADD_NET_CREATE_BTN }}</button>
      </div>
    </div>
    <div class="btn-row">
      <button v-if="outcome !== 'waiting'" type="button" class="btn" id="add-net-cancel" @click="cancel">{{ ADD_NET_CANCEL }}</button>
    </div>
    <div v-if="outcome === 'waiting' && form.slot" id="add-net-waiting">
      <JoiningCard :slot="form.slot" @cancelled="cancelled" />
    </div>
  </div>
</template>
