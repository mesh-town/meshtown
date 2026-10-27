<script setup lang="ts">
// The first wizard screen: join a network with an invite or create one. The form is driven by `useJoin`.
import { computed, inject, nextTick } from "vue";
import { STORE_KEY } from "../../core/store.ts";
import JoinCard from "../JoinCard.vue";
import { PROGRESS_HEAD, joinProgress } from "../join-progress.ts";
import { refusedLead } from "../join-wait.ts";
import { useJoin } from "../use-join.ts";
import { useRefusalGate } from "../use-refusal-gate.ts";
import {
  HOME_WIZARD_CREATE_BTN, HOME_WIZARD_INVITE_HINT, HOME_WIZARD_JOIN_BTN, HOME_WIZARD_NOT_JOINED,
} from "../home-card-texts.ts";

const store = inject(STORE_KEY)!;
const j = useJoin(store);
// Searching and joining replace the buttons with a progress block; a node refusal is text above the buttons.
const refusal = useRefusalGate(() => store.state?.node?.invite_join);
const maskedState = computed(() => ({ ...store.state, node: { ...store.state?.node, invite_join: refusal.info.value } }) as typeof store.state);
const progress = computed(() => joinProgress(maskedState.value, j.sent.value, store.absence ?? null));
const lead = computed(() => refusedLead(refusal.info.value));
const running = computed(() => (progress.value && progress.value.phase !== "refused" ? progress.value : null));

const focusField = (id: string): void => document.getElementById(id)?.focus();

async function startJoin(): Promise<void> {
  j.openJoin();
  await nextTick();
  focusField("join-key-input");
}

async function startCreate(): Promise<void> {
  j.openCreate();
  await nextTick();
  focusField("create-label-input");
}
</script>

<template>
  <div id="home-start-choice">
    <div v-if="running" class="join-progress" role="status" aria-live="polite">
      <span class="spinner spinner-big" aria-hidden="true"></span>
      <div><div class="join-progress-main" id="home-start-title">{{ running.main }}</div><div class="join-progress-sub">{{ running.sub }}</div></div>
    </div>
    <template v-else>
      <h3 id="home-start-title">{{ HOME_WIZARD_NOT_JOINED }}</h3>
      <p v-if="progress" class="field-error-text" id="join-refused" role="status" aria-live="polite"><strong v-if="lead">{{ lead }}</strong>{{ lead ? " " : "" }}{{ progress.main }}</p>
      <p class="hint">{{ HOME_WIZARD_INVITE_HINT }}</p>
      <div class="btn-row">
        <button type="button" class="btn btn-primary" id="home-start-join-btn" aria-controls="join-card" :aria-expanded="j.form.choice === 'join'" @click="startJoin">{{ HOME_WIZARD_JOIN_BTN }}</button>
        <button type="button" class="btn" id="home-start-create-btn" aria-controls="join-card" :aria-expanded="j.form.choice === 'create'" @click="startCreate">{{ HOME_WIZARD_CREATE_BTN }}</button>
      </div>
      <JoinCard v-if="j.form.choice" :join="j" quiet @attempt="refusal.dismiss" />
    </template>
  </div>
</template>
