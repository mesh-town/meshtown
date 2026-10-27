<script setup lang="ts">
// The "Three steps" setup (without a network or after joining/creating a network from this page): join, browser, exit.
// The join form is driven by the setup (`useJoin`), so that the buttons "Join the network with an invite" and "Create a new network"
// above open it.
import { computed, inject, nextTick, ref } from "vue";
import { quotedNet } from "../common/words.ts";
import { STORE_KEY } from "../core/store.ts";
import BrowserCard from "./BrowserCard.vue";
import { inAppPage } from "./browser-setup.ts";
import ExitCard from "./ExitCard.vue";
import InviteCard from "./InviteCard.vue";
import JoinCard from "./JoinCard.vue";
import NodeNameField from "./NodeNameField.vue";
import ThisComputerCard from "./ThisComputerCard.vue";
import type { SetupMark } from "./home-actions.ts";
import { shownStep } from "./home-view.ts";
import { useHomeContext } from "./use-home.ts";
import { PROGRESS_HEAD, joinProgress } from "./join-progress.ts";
import { useRefusalGate } from "./use-refusal-gate.ts";
import { useJoin } from "./use-join.ts";
import { refusedLead } from "./join-wait.ts";
import {
  HOME_WIZARD_BROWSER_DONE, HOME_WIZARD_CREATE_BTN, HOME_WIZARD_DONE, HOME_WIZARD_HEAD_JOIN, HOME_WIZARD_INVITE_HINT, HOME_WIZARD_JOIN_BTN,
  HOME_WIZARD_JOINED, HOME_WIZARD_JOINED_NOT_RUNNING, HOME_WIZARD_NOT_JOINED, HOME_WIZARD_STEP_BROWSER, HOME_WIZARD_STEP_CONNECT, HOME_WIZARD_STEP_EXIT, HOME_WIZARD_TITLE,
  HOME_PHONE_TITLE,
} from "./home-card-texts.ts";

const store = inject(STORE_KEY)!;
const { wizardStepNumber: rawStep, hasNet, networkLabel, ...h } = useHomeContext(store);
// The step chosen by clicking the title (in a network — 2 or 3); lives until "Done", doesn't write the mark.
const picked = ref<number | null>(null);
// In the Android app the browser needs no setup: the browser step is hidden and the exit step is numbered 2.
const inApp = inAppPage();
const step = computed(() => {
  const n = picked.value ?? shownStep(rawStep.value);
  return inApp && n === 2 ? 3 : n;
});
const pickable = (n: number): boolean => hasNet.value && step.value !== n;
const finish = (mark: SetupMark): void => {
  picked.value = null;
  h.setMark(mark);
};
// A failed start: the network is saved but the node is not running, so the browser and exit setup would point at nothing.
const startFailed = computed(() => !!store.state?.node?.start_error);
const j = useJoin(store);
// The join progress phase: searching and joining replace the buttons with a progress block; a node refusal is text above the buttons.
const refusal = useRefusalGate(() => store.state?.node?.invite_join);
const maskedState = computed(() => ({ ...store.state, node: { ...store.state?.node, invite_join: refusal.info.value } }) as typeof store.state);
const progress = computed(() => joinProgress(maskedState.value, j.sent.value, store.absence ?? null));
const lead = computed(() => refusedLead(refusal.info.value));
const running = computed(() => (progress.value && progress.value.phase !== "refused" ? progress.value : null));
const stepHead = computed(() => (running.value ? PROGRESS_HEAD[running.value.phase as "searching" | "entering"] : HOME_WIZARD_HEAD_JOIN));

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
  <div v-if="step === 1" class="card wizard-card" id="home-start-choice" role="region" aria-labelledby="home-start-title">
    <div v-if="running" class="join-progress" role="status" aria-live="polite">
      <span class="spinner spinner-big" aria-hidden="true"></span>
      <div><div class="join-progress-main" id="home-start-title">{{ running.main }}</div><div class="join-progress-sub">{{ running.sub }}</div></div>
    </div>
    <template v-else>
    <h1 id="home-start-title">{{ HOME_WIZARD_NOT_JOINED }}</h1>
    <p v-if="progress" class="field-error-text" id="join-refused" role="status" aria-live="polite"><strong v-if="lead">{{ lead }}</strong>{{ lead ? " " : "" }}{{ progress.main }}</p>
    <p class="hint">{{ HOME_WIZARD_INVITE_HINT }}</p>
    <div class="btn-row">
      <button type="button" :class="['btn', { 'btn-primary': !j.form.choice }]" id="home-start-join-btn" aria-controls="join-card" :aria-expanded="j.form.choice === 'join'" @click="startJoin">{{ HOME_WIZARD_JOIN_BTN }}</button>
      <button type="button" class="btn" id="home-start-create-btn" aria-controls="join-card" :aria-expanded="j.form.choice === 'create'" @click="startCreate">{{ HOME_WIZARD_CREATE_BTN }}</button>
    </div>
    <JoinCard v-if="j.form.choice" :join="j" quiet @attempt="refusal.dismiss" />
    </template>
  </div>
  <div class="card wizard-card" role="region" aria-labelledby="home-wizard-title">
    <h2 id="home-wizard-title">{{ HOME_WIZARD_TITLE }}</h2>
    <div class="wizard-step" :class="{ done: hasNet }">
      <h3 v-if="hasNet && startFailed">{{ HOME_WIZARD_JOINED_NOT_RUNNING }}</h3>
      <h3 v-else-if="hasNet">{{ HOME_WIZARD_JOINED }}{{ quotedNet(networkLabel) }}</h3>
      <div v-else class="step-head"><span v-if="running" class="spinner" aria-hidden="true"></span><span v-else class="step-num" aria-hidden="true">1</span><h3>{{ stepHead }}</h3></div>
    </div>
    <div v-if="!inApp" class="wizard-step" :class="{ pending: step < 2, done: step > 2 }">
      <div class="step-head"><h3><button type="button" class="step-pick" id="wizard-step-2" :disabled="!pickable(2)" :aria-current="step === 2 ? 'step' : undefined" @click="picked = 2"><span class="step-num" aria-hidden="true">2</span>{{ HOME_WIZARD_STEP_BROWSER }}</button></h3></div>
      <template v-if="step === 2 && !startFailed">
        <BrowserCard bare />
        <div class="btn-row"><button type="button" class="btn btn-primary" @click="finish('exit')">{{ HOME_WIZARD_BROWSER_DONE }}</button></div>
      </template>
    </div>
    <div class="wizard-step" :class="{ pending: step < 3 }">
      <div class="step-head"><h3><button type="button" class="step-pick" id="wizard-step-3" :disabled="!pickable(3)" :aria-current="step === 3 ? 'step' : undefined" @click="picked = 3"><span class="step-num" aria-hidden="true">{{ inApp ? 2 : 3 }}</span>{{ HOME_WIZARD_STEP_EXIT }}</button></h3></div>
      <template v-if="step === 3 && !startFailed">
        <ExitCard bare />
        <ThisComputerCard v-if="!inApp" bare />
        <div v-if="inApp" class="step-head step-static" id="wizard-connect-step"><span class="step-num" aria-hidden="true">3</span>{{ HOME_WIZARD_STEP_CONNECT }}</div>
        <div class="btn-row"><button type="button" class="btn btn-primary" @click="finish('done')">{{ HOME_WIZARD_DONE }}</button></div>
      </template>
    </div>
  </div>
  <div v-if="inApp && step > 1" class="card" id="home-phone-card" role="region" aria-labelledby="home-phone-title">
    <h2 id="home-phone-title">{{ HOME_PHONE_TITLE }}</h2>
    <NodeNameField with-save />
  </div>
  <InviteCard v-if="step > 1" />
</template>
