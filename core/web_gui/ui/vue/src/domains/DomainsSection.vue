<script setup lang="ts">
import { computed, inject, ref } from "vue";
import { saveExitIn } from "../core/exit-via.ts";
import type { ExitSaveStatus } from "../core/exit-via.ts";
import { STORE_KEY } from "../core/store.ts";
import type { Store } from "../core/store.ts";
import NoNetwork from "../common/NoNetwork.vue";
import { useNarrow } from "../common/use-narrow.ts";
import DomainCards from "./DomainCards.vue";
import DomainRow from "./DomainRow.vue";
import {
  DOMAINS_ADD, DOMAINS_ADDRESS_LABEL, DOMAINS_ADDRESS_PLACEHOLDER, DOMAINS_COL_SITE, DOMAINS_COL_OTHERS, DOMAINS_EMPTY,
  DOMAINS_HINT_INTRO, DOMAINS_NO_EXIT, DOMAINS_HINT_LOCAL, DOMAINS_LEGEND_TITLE, DOMAINS_LOADING, DOMAINS_NO_STORE, DOMAINS_TABLE_LABEL,
  domainsExitSavedText,
} from "./domains-texts.ts";
import { LEGEND, checkAllLabel, mergedDomains, proxySlot, routeExits, rowsBusy, siteNets } from "./domains-view.ts";
import { exitMemberName, exitPath, viaHead } from "./domains-summary.ts";
import { useDomainActions } from "./use-domain-actions.ts";

const injected = inject(STORE_KEY);
if (!injected) throw new Error(DOMAINS_NO_STORE);
const store: Store = injected;
const actions = useDomainActions(store);
const newDomain = ref<string>("");
const exitStatus = ref<ExitSaveStatus>({ text: "", cls: "" });

const narrow = useNarrow();
const nets = computed(() => siteNets(store));
const noExit = computed(() => nets.value.length > 0 && nets.value.every((n) => n.peers.length === 0));
const domains = computed(() => mergedDomains(nets.value, store));
const proxy = computed(() => proxySlot(nets.value, store.nets));
const exits = computed(() => routeExits(store, proxy.value));
const head = computed(() => viaHead(exitPath(nets.value, proxy.value, exits.value)));
const checkAllOff = computed(() => actions.ui.checkAllBusy || rowsBusy(domains.value));

async function add(): Promise<void> {
  if (await actions.add(newDomain.value)) newDomain.value = "";
}

// The "Exit via …" button makes the member the only exit.
async function exitVia(idFull: string): Promise<void> {
  const res = await saveExitIn(store, proxy.value, idFull);
  exitStatus.value = res.cls === "saved" ? { text: domainsExitSavedText(exitMemberName(nets.value, proxy.value, idFull)), cls: res.cls } : res;
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
      <p class="hint" id="domains-hint">{{ DOMAINS_HINT_INTRO }} {{ DOMAINS_HINT_LOCAL }}</p>
      <p class="hint" id="domains-no-exit" v-if="noExit">{{ DOMAINS_NO_EXIT }}</p>
      <p class="field-status" id="domains-check-all-status" role="status" :hidden="!actions.ui.checkAllStatus">{{ actions.ui.checkAllStatus }}</p>
      <p class="field-status" :class="exitStatus.cls" id="domains-exit-status" role="status" :hidden="!exitStatus.text">{{ exitStatus.text }}</p>
      <DomainCards v-if="narrow" :domains="domains" :nets="nets" :proxy="proxy" :exits="exits" :show-others="!noExit" :busy="actions.ui.busy" :errors="actions.ui.errors" @recheck="actions.recheck" @remove="actions.remove" @exit-via="exitVia" />
      <div v-else class="table-scroll">
        <table class="data domains-table" :aria-label="DOMAINS_TABLE_LABEL">
          <thead v-if="domains.length > 0">
            <tr id="domains-head-row">
              <th>{{ DOMAINS_COL_SITE }}</th>
              <th>{{ head.text }}<br><span class="hint">{{ head.sub }}</span></th>
              <th v-if="!noExit">{{ DOMAINS_COL_OTHERS }}</th>
              <th></th>
            </tr>
          </thead>
          <tbody id="domains-tbody">
            <tr v-if="domains.length === 0"><td :colspan="noExit ? 3 : 4" class="hint">{{ DOMAINS_EMPTY }}</td></tr>
            <DomainRow v-for="d in domains" :key="d.name" :domain="d" :nets="nets" :proxy="proxy" :exits="exits" :show-others="!noExit" :busy="!!actions.ui.busy[d.name]" :error="actions.ui.errors[d.name] || ''" @recheck="actions.recheck" @remove="actions.remove" @exit-via="exitVia" />
          </tbody>
        </table>
      </div>
      <details id="domains-legend" class="hint">
        <summary>{{ DOMAINS_LEGEND_TITLE }}</summary>
        <ul>
          <li v-for="e in LEGEND" :key="e.text"><span class="badge" :class="e.cls">{{ e.text }}</span> — {{ e.note }}</li>
        </ul>
      </details>
    </div>
  </template>
</template>
