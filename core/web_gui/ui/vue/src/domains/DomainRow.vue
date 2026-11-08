<script setup lang="ts">
import { computed, ref } from "vue";
import { exitViaText } from "../common/words.ts";
import type { ChosenExit } from "../core/api-types.ts";
import {
  DOMAINS_CHECKING_ROW, DOMAINS_DETAILS, DOMAINS_DETAILS_HIDE, DOMAINS_EXIT_NOT_SHARING, DOMAINS_EXIT_OFFLINE, DOMAINS_NOT_CHECKED,
  DOMAINS_RECHECK, DOMAINS_REMOVE, domainsAllSitesViaText, domainsRecheckLabelText, domainsRemoveLabelText,
} from "./domains-texts.ts";
import type { MergedDomain, SiteNet } from "./domains-view.ts";
import { exitPath, siteSummary } from "./domains-summary.ts";
import SiteMembers from "./SiteMembers.vue";

const props = withDefaults(
  defineProps<{
    domain: MergedDomain;
    nets: SiteNet[];
    proxy: string;
    exits: ChosenExit[];
    showOthers: boolean;
    busy?: boolean;
    error?: string;
  }>(),
  { busy: false, error: "" },
);
const emit = defineEmits<{
  (e: "recheck", name: string): void;
  (e: "remove", name: string): void;
  (e: "exit-via", idFull: string): void;
}>();

const open = ref(false);
const sum = computed(() => siteSummary(props.domain, props.nets, props.proxy, props.exits));
const current = computed(() => exitPath(props.nets, props.proxy, props.exits).idFull);
const disabled = computed(() => props.busy || props.domain.checking);
const cols = computed(() => (props.showOthers ? 4 : 3));
</script>

<template>
  <tr class="domain-row">
    <td class="mono">{{ domain.name }}</td>
    <td class="domain-result">
      <span v-if="sum.kind === 'checking'" class="hint">{{ DOMAINS_CHECKING_ROW }}</span>
      <span v-else-if="sum.kind === 'exit-offline'" class="badge badge-down">{{ DOMAINS_EXIT_OFFLINE }}</span>
      <span v-else-if="sum.kind === 'exit-not-sharing'" class="badge badge-down">{{ DOMAINS_EXIT_NOT_SHARING }}</span>
      <template v-else-if="sum.mark">
        <span class="badge" :class="sum.mark.cls" :title="sum.mark.title">{{ sum.mark.text }}</span>
        <span v-if="sum.kind === 'unknown'" class="hint"> {{ DOMAINS_NOT_CHECKED }}</span>
        <span v-else-if="sum.age" class="hint"> · {{ sum.age }}</span>
      </template>
    </td>
    <td v-if="showOthers" class="domain-others">
      <span class="hint">{{ sum.others }}</span>
      <button v-if="sum.button" type="button" class="btn btn-sm" :title="domainsAllSitesViaText(sum.button.name)" @click="emit('exit-via', sum.button.idFull)">{{ exitViaText(sum.button.name) }}</button>
    </td>
    <td class="domain-actions">
      <button type="button" class="btn-x btn-text" :aria-expanded="open" @click="open = !open">{{ open ? DOMAINS_DETAILS_HIDE : DOMAINS_DETAILS }}</button>
      <button type="button" class="btn-x" :title="DOMAINS_RECHECK" :aria-label="domainsRecheckLabelText(domain.name)" :disabled="disabled" @click="emit('recheck', domain.name)">⟳</button>
      <button type="button" class="btn-x" :title="DOMAINS_REMOVE" :aria-label="domainsRemoveLabelText(domain.name)" :disabled="disabled" @click="emit('remove', domain.name)">×</button>
      <div v-if="error" class="field-error-text" role="status">{{ error }}</div>
    </td>
  </tr>
  <tr v-if="open" class="domain-details">
    <td :colspan="cols"><SiteMembers :domain="domain" :nets="nets" :proxy="proxy" :current="current" @exit-via="emit('exit-via', $event)" /></td>
  </tr>
</template>
