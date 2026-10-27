<script setup lang="ts">
// The step-by-step setup: a strip of steps, all the questions on one page, then the exit screen, Back / Next. The path and the answers live in `useWizard`.
import { computed, inject } from "vue";
import { quotedNet } from "../common/words.ts";
import { STORE_KEY } from "../core/store.ts";
import { inAppPage } from "./browser-setup.ts";
import InviteCard from "./InviteCard.vue";
import NodeNameField from "./NodeNameField.vue";
import {
  HOME_PHONE_TITLE, HOME_WIZARD_DONE, HOME_WIZARD_JOINED, HOME_WIZARD_JOINED_NOT_RUNNING,
  HOME_WIZARD_STEP_CONNECT, HOME_WIZARD_TITLE,
} from "./home-card-texts.ts";
import { useHomeContext } from "./use-home.ts";
import ChoicesScreen from "./wizard/ChoicesScreen.vue";
import StepExit from "./wizard/StepExit.vue";
import StepJoin from "./wizard/StepJoin.vue";
import StepStrip from "./wizard/StepStrip.vue";
import { useWizard } from "./wizard/use-wizard.ts";
import { WIZ_BACK, WIZ_FINISH, WIZ_NEXT, WIZ_PROGRESS, WIZ_QUESTIONS, WIZ_SKIP } from "./wizard/wizard-texts.ts";

const store = inject(STORE_KEY)!;
const { hasNet, networkLabel } = useHomeContext(store);
const w = useWizard(store);
const inApp = inAppPage();
// A failed start: the network is saved but the node is not running, so the setup would point at nothing.
const startFailed = computed(() => hasNet.value && !!store.state?.node?.start_error);
const question = computed(() => (w.step.value === "join" ? "" : WIZ_QUESTIONS[w.step.value as "exit" | "connect"]));
const nextText = computed(() => ({ next: WIZ_NEXT, finish: WIZ_FINISH, done: HOME_WIZARD_DONE, skip: "" })[w.nav.value ?? "next"]);
</script>

<template>
  <div class="card wizard-card" role="region" aria-labelledby="home-wizard-title">
    <h2 id="home-wizard-title">{{ HOME_WIZARD_TITLE }}</h2>
    <div v-if="startFailed" class="wizard-step failed"><h3>{{ HOME_WIZARD_JOINED_NOT_RUNNING }}</h3></div>
    <template v-else>
      <div class="wizard-body"><div class="wizard-layout">
        <StepStrip :steps="w.strip.value" :has-net="hasNet" @go="w.goStep" />
        <div class="wizard-main">
          <p id="wizard-progress" class="sr-only">{{ WIZ_PROGRESS(w.progress.value.n, w.progress.value.m) }}</p>
          <p v-if="w.joined.value" id="wizard-joined">{{ HOME_WIZARD_JOINED }}{{ quotedNet(networkLabel) }}</p>
          <div id="wizard-screen" :data-step="w.screen.value">
            <ChoicesScreen v-if="w.screen.value === 'choices'" :w="w" />
            <template v-else>
              <h3 v-if="w.step.value !== 'join'" id="wizard-q" tabindex="-1">{{ question }}</h3>
              <StepJoin v-if="w.step.value === 'join'" />
              <StepExit v-else-if="w.step.value === 'exit'" :in-app="inApp" />
              <p v-else-if="w.step.value === 'connect'" id="wizard-connect-step">{{ HOME_WIZARD_STEP_CONNECT }}</p>
            </template>
          </div>
          <div v-if="w.step.value !== 'join' && (w.back.value || w.nav.value)" class="wizard-nav">
            <button v-if="w.back.value" type="button" class="link-more" id="wizard-back" @click="w.goBack"><span aria-hidden="true">←</span> {{ WIZ_BACK }}</button>
            <button v-if="w.nav.value === 'skip'" type="button" class="link-more" id="wizard-skip" @click="w.skip">{{ WIZ_SKIP }}</button>
            <button v-else-if="w.nav.value" type="button" class="btn btn-primary" id="wizard-next" @click="w.nav.value === 'next' ? w.goNext() : w.finish()">{{ nextText }}</button>
          </div>
        </div>
      </div></div>
    </template>
  </div>
  <div v-if="inApp && hasNet" class="card" id="home-phone-card" role="region" aria-labelledby="home-phone-title">
    <h2 id="home-phone-title">{{ HOME_PHONE_TITLE }}</h2>
    <NodeNameField with-save />
  </div>
  <InviteCard v-if="hasNet" quiet />
</template>
