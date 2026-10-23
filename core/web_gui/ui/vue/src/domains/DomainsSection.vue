<script setup lang="ts">
import { computed, inject, ref } from "vue";
import { saveSingleExit } from "../core/exit-via.ts";
import type { ExitSaveStatus } from "../core/exit-via.ts";
import { STORE_KEY } from "../core/store.ts";
import type { Store } from "../core/store.ts";
import NoNetwork from "../common/NoNetwork.vue";
import { chosenExitIds } from "../common/exits.ts";
import { NET_NAME } from "../common/words.ts";
import { useNarrow } from "../common/use-narrow.ts";
import DomainCards from "./DomainCards.vue";
import DomainRow from "./DomainRow.vue";
import {
  DOMAINS_ADD, DOMAINS_ADDRESS_LABEL, DOMAINS_ADDRESS_PLACEHOLDER, DOMAINS_COL_SITE, DOMAINS_COL_YOU, DOMAINS_EMPTY,
  DOMAINS_HINT_INTRO, DOMAINS_HINT_LOCAL, DOMAINS_HINT_LOCAL_NETS, DOMAINS_HINT_REST, DOMAINS_LOADING, DOMAINS_NO_STORE, DOMAINS_TABLE_LABEL,
} from "./domains-texts.ts";
import { LEGEND, checkAllLabel, columnGroups, columnHead, mergedDomains, proxySlot, rowsBusy, siteNets, tableColspan } from "./domains-view.ts";
import { useDomainActions } from "./use-domain-actions.ts";

const injected = inject(STORE_KEY);
if (!injected) throw new Error(DOMAINS_NO_STORE);
const store: Store = injected;
const actions = useDomainActions(store);
const newDomain = ref<string>("");
const exitStatus = ref<ExitSaveStatus>({ text: "", cls: "" });

const narrow = useNarrow();
const nets = computed(() => siteNets(store));
const groups = computed(() => columnGroups(nets.value));
const domains = computed(() => mergedDomains(nets.value, store));
const proxy = computed(() => proxySlot(nets.value, store.nets));
const chosen = computed(() => chosenExitIds(store));
const checkAllOff = computed(() => actions.ui.checkAllBusy || rowsBusy(domains.value));

async function add(): Promise<void> {
  if (await actions.add(newDomain.value)) newDomain.value = "";
}

// The "Exit via …" button makes the member the only exit.
async function exitVia(idFull: string): Promise<void> {
  exitStatus.value = await saveSingleExit(store, idFull);
}
</script>

<template>
  <p v-if="!store.state" class="hint" id="domains-loading">{{ DOMAINS_LOADING }}</p>
  <template v-else>
    <NoNetwork v-if="!store.state.network" />
    <div v-else class="card">
      <div class="row-inline">
        <input v-model="newDomain" type="text" class="input" :class="{ 'field-error': actions.ui.addError }" id="domain-add-input" :aria-label="DOMAINS_ADDRESS_LABEL" aria-describedby="domain-add-error" :placeholder="DOMAINS_ADDRESS_PLACEHOLDER">
        <button type="button" class="btn btn-primary" id="domain-add-btn" @click="add">{{ DOMAINS_ADD }}</button>
        <span class="spacer"></span>
        <button type="button" class="btn" id="domains-check-all-btn" :disabled="checkAllOff" @click="actions.checkAll">{{ checkAllLabel(checkAllOff) }}</button>
      </div>
      <p class="field-error-text" id="domain-add-error" role="alert" :hidden="!actions.ui.addError">{{ actions.ui.addError }}</p>
      <p class="field-status" id="domains-check-all-status" role="status" :hidden="!actions.ui.checkAllStatus">{{ actions.ui.checkAllStatus }}</p>
      <DomainCards v-if="narrow" :domains="domains" :nets="nets" :proxy="proxy" :chosen="chosen" :busy="actions.ui.busy" :errors="actions.ui.errors" @recheck="actions.recheck" @remove="actions.remove" @exit-via="exitVia" />
      <div v-else class="table-scroll">
        <table class="data domains-table" :aria-label="DOMAINS_TABLE_LABEL">
          <thead>
            <tr v-if="nets.length >= 2" id="domains-groups-row">
              <th></th>
              <th></th>
              <th v-for="g in groups" :key="g.slot" :colspan="g.span" class="col-net">{{ g.label }}</th>
              <th></th>
            </tr>
            <tr id="domains-head-row">
              <th>{{ DOMAINS_COL_SITE }}</th>
              <th>{{ DOMAINS_COL_YOU }}</th>
              <template v-for="g in groups" :key="g.slot">
                <th v-for="p in g.peers" :key="p.id" class="col-peer" :title="p.id">{{ columnHead(p).name }}<br><span class="peer-code mono">{{ columnHead(p).id }}</span></th>
                <th v-if="g.peers.length === 0" class="col-peer">—</th>
              </template>
              <th></th>
            </tr>
          </thead>
          <tbody id="domains-tbody">
            <tr v-if="domains.length === 0"><td :colspan="tableColspan(groups)" class="hint">{{ DOMAINS_EMPTY }}</td></tr>
            <DomainRow v-for="d in domains" :key="d.name" :domain="d" :nets="nets" :proxy="proxy" :chosen="chosen" :busy="!!actions.ui.busy[d.name]" :error="actions.ui.errors[d.name] || ''" @recheck="actions.recheck" @remove="actions.remove" @exit-via="exitVia" />
          </tbody>
        </table>
      </div>
      <p class="field-status" :class="exitStatus.cls" id="domains-exit-status" role="status" :hidden="!exitStatus.text">{{ exitStatus.text }}</p>
      <p class="hint">{{ DOMAINS_HINT_INTRO }} {{ NET_NAME }}. {{ DOMAINS_HINT_REST }}</p>
      <ul class="hint" id="domains-legend">
        <li v-for="e in LEGEND" :key="e.text"><span class="badge" :class="e.cls">{{ e.text }}</span> — {{ e.note }}</li>
      </ul>
      <p class="hint">{{ nets.length >= 2 ? DOMAINS_HINT_LOCAL_NETS : DOMAINS_HINT_LOCAL }}</p>
    </div>
  </template>
</template>
