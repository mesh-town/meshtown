<script setup lang="ts">
// The step-by-step setup: a progress line, one question per screen, Back / Next. The path and the answers live in `useWizard`.
import { computed, inject, nextTick, watch } from "vue";
import { PAC_OS_LIST } from "./pac-setup.ts";
import { SCOPE_BROWSER, SCOPE_SYSTEM } from "../common/pac-texts.ts";
import { quotedNet } from "../common/words.ts";
import { STORE_KEY } from "../core/store.ts";
import { inAppPage } from "./browser-setup.ts";
import InviteCard from "./InviteCard.vue";
import NodeNameField from "./NodeNameField.vue";
import {
  HOME_PHONE_TITLE, HOME_PROXY_OFF, HOME_PROXY_OFF_LINK, HOME_WIZARD_DONE, HOME_WIZARD_JOINED, HOME_WIZARD_JOINED_NOT_RUNNING,
  HOME_WIZARD_STEP_CONNECT, HOME_WIZARD_TITLE,
} from "./home-card-texts.ts";
import { useHomeContext } from "./use-home.ts";
import StepChoice from "./wizard/StepChoice.vue";
import StepExit from "./wizard/StepExit.vue";
import StepGuide from "./wizard/StepGuide.vue";
import StepJoin from "./wizard/StepJoin.vue";
import { browserTiles } from "./wizard/guide-content.ts";
import { useWizard } from "./wizard/use-wizard.ts";
import { WIZ_BACK, WIZ_FINISH, WIZ_NEXT, WIZ_PROGRESS, WIZ_QUESTIONS, WIZ_SKIP } from "./wizard/wizard-texts.ts";

const store = inject(STORE_KEY)!;
const { hasNet, networkLabel } = useHomeContext(store);
const w = useWizard(store);
const inApp = inAppPage();
// A failed start: the network is saved but the node is not running, so the setup would point at nothing.
const startFailed = computed(() => hasNet.value && !!store.state?.node?.start_error);
const scopeOptions = [
  { id: "system", domId: "scope-system", label: SCOPE_SYSTEM },
  { id: "browser", domId: "scope-browser", label: SCOPE_BROWSER },
];
const osOptions = PAC_OS_LIST.map(([id, label]) => ({ id, domId: `os-${id}`, label }));
const browserOptions = computed(() => browserTiles(w.os.value).map(([id, label]) => ({ id, domId: `browser-${id}`, label })));
const question = computed(() => {
  const s = w.step.value;
  if (s === "guide") return w.guide.value.title;
  return s === "join" ? "" : WIZ_QUESTIONS[s];
});
const nextText = computed(() => ({ next: WIZ_NEXT, finish: WIZ_FINISH, done: HOME_WIZARD_DONE, skip: "" })[w.nav.value ?? "next"]);

// Move the focus to the question after a step change (not on mount).
watch(w.step, async () => {
  await nextTick();
  document.getElementById("wizard-q")?.focus();
});
</script>

<template>
  <div class="card wizard-card" role="region" aria-labelledby="home-wizard-title">
    <h2 id="home-wizard-title">{{ HOME_WIZARD_TITLE }}</h2>
    <div v-if="startFailed" class="wizard-step failed"><h3>{{ HOME_WIZARD_JOINED_NOT_RUNNING }}</h3></div>
    <template v-else>
      <p id="wizard-progress">{{ WIZ_PROGRESS(w.progress.value.n, w.progress.value.m) }}</p>
      <p v-if="w.joined.value" id="wizard-joined">{{ HOME_WIZARD_JOINED }}{{ quotedNet(networkLabel) }}</p>
      <div id="wizard-screen" :data-step="w.step.value">
        <h3 v-if="w.step.value !== 'join'" id="wizard-q" tabindex="-1">{{ question }}</h3>
        <StepJoin v-if="w.step.value === 'join'" />
        <template v-else-if="w.step.value === 'scope'">
          <p v-if="!w.ctx.value.proxyOn" id="proxy-hint">{{ HOME_PROXY_OFF }} <a href="#settings">{{ HOME_PROXY_OFF_LINK }}</a></p>
          <StepChoice v-else group-id="scope-seg" :options="scopeOptions" :picked="w.scope.value" @pick="(id) => w.pickScope(id as 'system' | 'browser')" />
        </template>
        <StepChoice v-else-if="w.step.value === 'os'" group-id="os-seg" :options="osOptions" :picked="w.os.value" @pick="(id) => w.pickOs(id as typeof w.os.value)" />
        <StepChoice v-else-if="w.step.value === 'browser'" group-id="browser-seg" :options="browserOptions" :picked="w.browser.value" @pick="(id) => w.pickBrowser(id as typeof w.browser.value)" />
        <StepGuide v-else-if="w.step.value === 'guide'" :guide="w.guide.value" @to-system="w.toSystem" />
        <StepExit v-else-if="w.step.value === 'exit'" :in-app="inApp" />
        <p v-else-if="w.step.value === 'connect'" id="wizard-connect-step">{{ HOME_WIZARD_STEP_CONNECT }}</p>
      </div>
      <div v-if="w.step.value !== 'join'" class="wizard-nav">
        <button v-if="w.back.value" type="button" class="link-more" id="wizard-back" @click="w.goBack">{{ WIZ_BACK }}</button>
        <button v-if="w.nav.value === 'skip'" type="button" class="link-more" id="wizard-skip" @click="w.skip">{{ WIZ_SKIP }}</button>
        <button v-else-if="w.nav.value" type="button" class="btn btn-primary" id="wizard-next" @click="w.nav.value === 'next' ? w.goNext() : w.finish()">{{ nextText }}</button>
      </div>
    </template>
  </div>
  <div v-if="inApp && hasNet" class="card" id="home-phone-card" role="region" aria-labelledby="home-phone-title">
    <h2 id="home-phone-title">{{ HOME_PHONE_TITLE }}</h2>
    <NodeNameField with-save />
  </div>
  <InviteCard v-if="hasNet" quiet />
</template>
