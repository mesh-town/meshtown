<script setup lang="ts">
// One site as a card: the name, the result for your browser, what to do, the text buttons, the details.
import { computed, ref } from "vue";
import { exitViaText } from "../common/words.ts";
import type { ChosenExit } from "../core/api-types.ts";
import {
  DOMAINS_DETAILS, DOMAINS_DETAILS_HIDE, DOMAINS_RECHECK, DOMAINS_REMOVE, domainsAllSitesViaText, domainsRecheckLabelText, domainsRemoveLabelText,
} from "./domains-texts.ts";
import type { MergedDomain, SiteNet } from "./domains-view.ts";
import { exitPath, siteSummary, viaHead } from "./domains-summary.ts";
import SiteMembers from "./SiteMembers.vue";
import SiteResult from "./SiteResult.vue";

const props = withDefaults(
  defineProps<{ domain: MergedDomain; nets: SiteNet[]; proxy: string; exits: ChosenExit[]; showOthers: boolean; busy?: boolean; error?: string }>(),
  { busy: false, error: "" },
);
const emit = defineEmits<{
  (e: "recheck", name: string): void;
  (e: "remove", name: string): void;
  (e: "exit-via", idFull: string): void;
}>();

const open = ref(false);
const path = computed(() => exitPath(props.nets, props.proxy, props.exits));
const sum = computed(() => siteSummary(props.domain, props.nets, props.proxy, props.exits));
const head = computed(() => viaHead(path.value).text);
const disabled = computed(() => props.busy || props.domain.checking);
</script>

<template>
  <div class="domain-card">
    <div class="mono domain-card-name">{{ domain.name }}</div>
    <div class="domain-card-result">{{ head }}: <SiteResult :sum="sum" /></div>
    <div v-if="showOthers" class="domain-card-others">
      <span class="hint">{{ sum.others }}</span>
      <button v-if="sum.button" type="button" class="btn btn-sm" :title="domainsAllSitesViaText(sum.button.name)" @click="emit('exit-via', sum.button.idFull)">{{ exitViaText(sum.button.name) }}</button>
    </div>
    <div class="domain-card-foot">
      <button type="button" class="btn-x btn-text" :aria-expanded="open" @click="open = !open">{{ open ? DOMAINS_DETAILS_HIDE : DOMAINS_DETAILS }}</button>
      <button type="button" class="btn-x btn-text" :aria-label="domainsRecheckLabelText(domain.name)" :disabled="disabled" @click="emit('recheck', domain.name)">{{ DOMAINS_RECHECK }}</button>
      <button type="button" class="btn-x btn-text" :aria-label="domainsRemoveLabelText(domain.name)" :disabled="disabled" @click="emit('remove', domain.name)">{{ DOMAINS_REMOVE }}</button>
    </div>
    <div v-if="error" class="field-error-text" role="status">{{ error }}</div>
    <SiteMembers v-if="open" :domain="domain" :nets="nets" :proxy="proxy" :current="path.idFull" @exit-via="emit('exit-via', $event)" />
  </div>
</template>
