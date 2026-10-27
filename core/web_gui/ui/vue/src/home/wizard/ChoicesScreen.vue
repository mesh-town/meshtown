<script setup lang="ts">
// The questions page of the setup wizard: the rows down to the cursor, one under another; a row opens the next one when answered.
import { computed } from "vue";
import { SCOPE_BROWSER, SCOPE_SYSTEM } from "../../common/pac-texts.ts";
import { HOME_PROXY_OFF, HOME_PROXY_OFF_LINK } from "../home-card-texts.ts";
import { PAC_OS_LIST } from "../pac-setup.ts";
import StepChoice from "./StepChoice.vue";
import StepGuide from "./StepGuide.vue";
import { browserTiles } from "./guide-content.ts";
import type { WizardView } from "./use-wizard.ts";
import { WIZ_QUESTIONS } from "./wizard-texts.ts";

const props = defineProps<{ w: WizardView }>();
const scopeOptions = [
  { id: "system", domId: "scope-system", label: SCOPE_SYSTEM },
  { id: "browser", domId: "scope-browser", label: SCOPE_BROWSER },
];
const osOptions = PAC_OS_LIST.map(([id, label]) => ({ id, domId: `os-${id}`, label }));
const browserOptions = computed(() => browserTiles(props.w.os.value).map(([id, label]) => ({ id, domId: `browser-${id}`, label })));
const title = (id: string): string => (id === "guide" ? props.w.guide.value.title : WIZ_QUESTIONS[id as "scope" | "os" | "browser"]);
// A row above the cursor already has its answer: its heading is quieter.
const answeredRow = (id: string): boolean => id !== props.w.step.value;
</script>

<template>
  <section v-for="id in w.rows.value" :key="id" :class="['wizard-row', { answered: answeredRow(id) }]" :data-step="id">
    <h3 :id="`wizard-q-${id}`" tabindex="-1">{{ title(id) }}</h3>
    <template v-if="id === 'scope'">
      <p v-if="!w.ctx.value.proxyOn" id="proxy-hint">{{ HOME_PROXY_OFF }} <a href="#settings">{{ HOME_PROXY_OFF_LINK }}</a></p>
      <StepChoice v-else group-id="scope-seg" label-id="wizard-q-scope" :options="scopeOptions" :picked="w.scope.value" @pick="(s) => w.pickScope(s as 'system' | 'browser')" />
    </template>
    <StepChoice v-else-if="id === 'os'" group-id="os-seg" label-id="wizard-q-os" :options="osOptions" :picked="w.os.value" @pick="(o) => w.pickOs(o as typeof w.os.value)" />
    <StepChoice v-else-if="id === 'browser'" group-id="browser-seg" label-id="wizard-q-browser" :options="browserOptions" :picked="w.browser.value" @pick="(b) => w.pickBrowser(b as typeof w.browser.value)" />
    <StepGuide v-else :guide="w.guide.value" @to-system="w.toSystem" />
  </section>
</template>
