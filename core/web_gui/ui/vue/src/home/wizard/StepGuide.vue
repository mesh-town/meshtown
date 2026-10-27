<script setup lang="ts">
// The guide screen: the PAC address with copying, the numbered steps for the chosen system and browser, extras.
import { computed, inject } from "vue";
import { STORE_KEY } from "../../core/store.ts";
import { isRouter } from "../../lan/nets-view.ts";
import NetworksSummary from "../../lan/NetworksSummary.vue";
import { HOME_COPIED, HOME_COPY_ADDR } from "../home-card-texts.ts";
import { copyAriaLabel } from "../pac-setup.ts";
import { useHomeContext } from "../use-home.ts";
import type { Guide } from "./guide-content.ts";
import MoreOptions from "./MoreOptions.vue";
import TermText from "./TermText.vue";
import { WIZ_APP_LINK, WIZ_COPY_ITEM, WIZ_TO_SYSTEM } from "./wizard-texts.ts";

defineProps<{ guide: Guide }>();
defineEmits<{ toSystem: [] }>();
const store = inject(STORE_KEY)!;
const { form, proxy, ...h } = useHomeContext(store);
const router = computed(() => isRouter(store.state ?? {}));
</script>

<template>
  <p v-if="guide.intro" id="guide-intro"><TermText :text="guide.intro" /></p>
  <ol v-if="guide.copy || guide.steps.length" id="pac-steps">
    <li v-if="guide.copy && proxy.on">
      <TermText :text="WIZ_COPY_ITEM" /> <code class="mono" id="pac-url-text">{{ proxy.pacUrl }}</code> <button type="button" class="btn btn-sm" id="pac-copy-btn" :aria-label="copyAriaLabel('pac', form.browser.copied)" @click="h.copy('pac')">{{ form.browser.copied === "pac" ? HOME_COPIED : HOME_COPY_ADDR }}</button>
      <p class="hint" id="pac-copy-note" :hidden="form.browser.noteKind !== 'pac' || !form.browser.note">{{ form.browser.note }}</p>
      <NetworksSummary v-if="router" />
    </li>
    <li v-for="(s, i) in guide.steps" :key="i">
      <TermText :text="s.text" /><template v-if="s.href"> <a id="guide-app-link" :href="s.href" target="_blank" rel="noopener">{{ WIZ_APP_LINK }}</a></template>
    </li>
  </ol>
  <p v-for="(t, i) in guide.small" :key="'s' + i" class="hint pac-small"><TermText :text="t" /></p>
  <template v-if="guide.browserSteps.length">
    <h4 id="guide-browser-title">{{ guide.browserTitle }}</h4>
    <ol id="browser-steps">
      <li v-for="(t, i) in guide.browserSteps" :key="i"><TermText :text="t" /></li>
    </ol>
    <p v-for="(t, i) in guide.browserSmall" :key="'b' + i" class="hint pac-small"><TermText :text="t" /></p>
  </template>
  <button v-if="guide.toSystem" type="button" class="btn" id="guide-to-system" @click="$emit('toSystem')">{{ WIZ_TO_SYSTEM }}</button>
  <MoreOptions v-if="guide.more" />
</template>
