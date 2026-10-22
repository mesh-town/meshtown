<script setup lang="ts">
import { computed } from "vue";
import type { DomainInfo, PeerInfo } from "../core/api-types.ts";
import { exitViaText, peerName } from "../common/words.ts";
import { DOMAINS_CHECKING_ROW, DOMAINS_CHOSEN_PREFIX, DOMAINS_RECHECK, DOMAINS_REMOVE, domainsAllSitesViaText, domainsRecheckLabelText, domainsRemoveLabelText } from "./domains-texts.ts";
import { exitCell, verdictView } from "./domains-view.ts";

const props = withDefaults(
  defineProps<{
    domain: DomainInfo;
    columns: string[];
    others: PeerInfo[];
    selfId: string;
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

const marks = computed(() => props.columns.map((id) => verdictView((props.domain.checks || {})[id])));
const cell = computed(() => exitCell(props.domain, props.others, props.selfId, props.chosen));
const via = computed(() => (cell.value.kind !== "none" ? peerName(cell.value.peer) : ""));
const disabled = computed(() => props.busy || props.domain.checking === true);
</script>

<template>
  <tr>
    <td class="mono">{{ domain.name }} <span v-if="domain.checking" class="hint">{{ DOMAINS_CHECKING_ROW }}</span></td>
    <td v-for="(m, i) in marks" :key="columns[i]"><span class="badge" :class="m.cls" :title="m.title">{{ m.text }}</span></td>
    <td>
      <span v-if="cell.kind === 'chosen'" class="peer-chosen">{{ DOMAINS_CHOSEN_PREFIX }} {{ via }}</span>
      <button v-else-if="cell.kind === 'button'" type="button" class="btn btn-sm" :title="domainsAllSitesViaText(via)" @click="emit('exit-via', cell.peer.id_full)">{{ exitViaText(via) }}</button>
      <button type="button" class="btn-x" :title="DOMAINS_RECHECK" :aria-label="domainsRecheckLabelText(domain.name)" :disabled="disabled" @click="emit('recheck', domain.name)">⟳</button>
      <button type="button" class="btn-x" :title="DOMAINS_REMOVE" :aria-label="domainsRemoveLabelText(domain.name)" :disabled="disabled" @click="emit('remove', domain.name)">×</button>
      <div v-if="error" class="field-error-text" role="status">{{ error }}</div>
    </td>
  </tr>
</template>
