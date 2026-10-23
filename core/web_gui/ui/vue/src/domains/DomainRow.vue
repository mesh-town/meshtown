<script setup lang="ts">
import { computed } from "vue";
import { exitViaText, peerName } from "../common/words.ts";
import { DOMAINS_CHECKING_ROW, DOMAINS_CHOSEN_PREFIX, DOMAINS_RECHECK, DOMAINS_REMOVE, domainsAllSitesViaText, domainsRecheckLabelText, domainsRemoveLabelText } from "./domains-texts.ts";
import { domainCells, domainExit } from "./domains-view.ts";
import type { MergedDomain, SiteNet } from "./domains-view.ts";

const props = withDefaults(
  defineProps<{
    domain: MergedDomain;
    nets: SiteNet[];
    proxy: string;
    chosen: string[];
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

const cells = computed(() => domainCells(props.domain, props.nets));
const cell = computed(() => domainExit(props.domain, props.nets, props.proxy, props.chosen));
const via = computed(() => (cell.value.kind !== "none" ? peerName(cell.value.peer) : ""));
const disabled = computed(() => props.busy || props.domain.checking);
</script>

<template>
  <tr>
    <td class="mono">{{ domain.name }} <span v-if="domain.checking" class="hint">{{ DOMAINS_CHECKING_ROW }}</span></td>
    <td v-for="c in cells" :key="c.key"><span class="badge" :class="c.mark.cls" :title="c.mark.title">{{ c.mark.text }}</span></td>
    <td>
      <span v-if="cell.kind === 'chosen'" class="peer-chosen">{{ DOMAINS_CHOSEN_PREFIX }} {{ via }}</span>
      <button v-else-if="cell.kind === 'button'" type="button" class="btn btn-sm" :title="domainsAllSitesViaText(via)" @click="emit('exit-via', cell.peer.id_full)">{{ exitViaText(via) }}</button>
      <button type="button" class="btn-x" :title="DOMAINS_RECHECK" :aria-label="domainsRecheckLabelText(domain.name)" :disabled="disabled" @click="emit('recheck', domain.name)">⟳</button>
      <button type="button" class="btn-x" :title="DOMAINS_REMOVE" :aria-label="domainsRemoveLabelText(domain.name)" :disabled="disabled" @click="emit('remove', domain.name)">×</button>
      <div v-if="error" class="field-error-text" role="status">{{ error }}</div>
    </td>
  </tr>
</template>
