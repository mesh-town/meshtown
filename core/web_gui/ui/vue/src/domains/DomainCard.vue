<script setup lang="ts">
// One site as a card: the name with the text buttons, then a row for you and one per member.
import { computed } from "vue";
import { EXIT_VIA_HIM, peerName } from "../common/words.ts";
import { DOMAINS_CHECKING_ROW, DOMAINS_COL_YOU, DOMAINS_NOT_CHECKED_CARD, DOMAINS_RECHECK, DOMAINS_REMOVE, DOMAINS_YOUR_EXIT, domainsAllSitesViaText, domainsRecheckLabelText, domainsRemoveLabelText } from "./domains-texts.ts";
import { domainCells, domainExit } from "./domains-view.ts";
import type { MergedDomain, SiteNet } from "./domains-view.ts";

const props = withDefaults(
  defineProps<{ domain: MergedDomain; nets: SiteNet[]; proxy: string; chosen: string[]; busy?: boolean; error?: string }>(),
  { busy: false, error: "" },
);
const emit = defineEmits<{
  (e: "recheck", name: string): void;
  (e: "remove", name: string): void;
  (e: "exit-via", idFull: string): void;
}>();

const disabled = computed(() => props.busy || props.domain.checking);
const cell = computed(() => domainExit(props.domain, props.nets, props.proxy, props.chosen));
// Networks without members have no row on the card.
const rows = computed(() =>
  domainCells(props.domain, props.nets)
    .filter((c) => c.kind !== "empty")
    .map((c) => {
      const own = c.peer !== null && c.slot === props.proxy && cell.value.kind !== "none" && cell.value.peer.id_full === c.peer.id_full;
      return { ...c, label: c.peer ? peerName(c.peer) : DOMAINS_COL_YOU, button: own && cell.value.kind === "button", isChosen: own && cell.value.kind === "chosen" };
    }),
);
</script>

<template>
  <div class="domain-card">
    <div class="domain-card-head">
      <span class="mono domain-card-name">{{ domain.name }} <span v-if="domain.checking" class="hint">{{ DOMAINS_CHECKING_ROW }}</span></span>
      <button type="button" class="btn-x btn-text" :aria-label="domainsRecheckLabelText(domain.name)" :disabled="disabled" @click="emit('recheck', domain.name)">{{ DOMAINS_RECHECK }}</button>
      <button type="button" class="btn-x btn-text" :aria-label="domainsRemoveLabelText(domain.name)" :disabled="disabled" @click="emit('remove', domain.name)">{{ DOMAINS_REMOVE }}</button>
    </div>
    <div v-for="r in rows" :key="r.key" class="domain-card-row">
      <span class="domain-card-who">{{ r.label }}<span v-if="r.tail" class="hint"> {{ r.tail }}</span></span>
      <span v-if="!r.checked" class="hint">{{ DOMAINS_NOT_CHECKED_CARD }}</span>
      <span v-else class="badge" :class="r.mark.cls" :title="r.mark.title">{{ r.mark.text }}</span>
      <span v-if="r.isChosen" class="peer-chosen">{{ DOMAINS_YOUR_EXIT }}</span>
      <button v-else-if="r.button && r.peer" type="button" class="btn btn-sm" :title="domainsAllSitesViaText(r.label)" @click="emit('exit-via', r.peer.id_full)">{{ EXIT_VIA_HIM }}</button>
    </div>
    <div v-if="error" class="field-error-text" role="status">{{ error }}</div>
  </div>
</template>
