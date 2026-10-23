<script setup lang="ts">
import { computed, inject, ref } from "vue";
import { STORE_KEY } from "../core/store.ts";
import type { Store } from "../core/store.ts";
import { isMulti } from "../core/slots.ts";
import type { NetPort, OwnPort, PeerInfo } from "../core/api-types.ts";
import NetPortRow from "./NetPortRow.vue";
import OwnPortRow from "./OwnPortRow.vue";
import PublishForm from "./PublishForm.vue";
import type { MergedOwn, NetPortItem } from "./ports-view.ts";
import {
  EMPTY_NET, EMPTY_OWN, audienceText, configPorts, localUsedText, mergeOwn, mergedAudienceText, netGroups, netPortItems, netRowKey, nodeLabel, sameNameNodes, takenLocals,
} from "./ports-view.ts";
import { usePortsActions } from "./use-ports-actions.ts";
import { PORTS_COL_AUDIENCE, PORTS_COL_BROWSER, PORTS_COL_NAME, PORTS_COL_PEER, PORTS_COL_PORT, PORTS_COL_PROGRAMS, PORTS_COL_STATUS, PORTS_MINE_HINT, PORTS_MINE_TITLE, PORTS_NET_HINTS, PORTS_NET_TITLE, PORTS_NO_STORE, PORTS_SAME_NAME_HINT } from "./ports-texts.ts";

const injected = inject(STORE_KEY);
if (!injected) throw new Error(PORTS_NO_STORE);
const store: Store = injected;
const { saveStatus, addError, editError, saving, openErrors, publish, edit, unpublish, openNetPort, closeNetPort } = usePortsActions(store);

const multi = computed<boolean>(() => isMulti(store));
// One network: the rows of the state as before. Several: one row per port number over all networks.
const own = computed<Pick<OwnPort, "port" | "name" | "local">[]>(() => (multi.value ? mergeOwn(store) : (store.state && store.state.ports.published) || []));
const merged = computed<MergedOwn[]>(() => (multi.value ? mergeOwn(store) : []));
const net = computed<NetPortItem[]>(() => netPortItems(store));
const groups = computed(() => (multi.value ? netGroups(store) : undefined));

const peers = computed<PeerInfo[]>(() => (store.state && store.state.peers) || []);

// The port whose editor is open now (one for the whole table).
const editing = ref<number | null>(null);

function entryOf(port: number) {
  return configPorts(store.config, "published").find((p) => p.port === port);
}

function mergedOf(port: number): MergedOwn | undefined {
  return merged.value.find((m) => m.port === port);
}

function audienceOfPort(port: number): string {
  const m = mergedOf(port);
  return m ? mergedAudienceText(store, m) : audienceText(entryOf(port), peers.value);
}

// What each network shares now; the editor's groups also keep the ids of members who left.
function sharedOf(port: number): Record<string, "all" | string[]> | undefined {
  const m = mergedOf(port);
  return m && Object.fromEntries(m.shared.map((s) => [s.slot, s.audience]));
}

function editGroups(port: number) {
  const extra = Object.fromEntries(Object.entries(sharedOf(port) || {}).map(([slot, a]) => [slot, a === "all" ? [] : a]));
  return multi.value ? netGroups(store, extra) : undefined;
}

// The result of the previous action ("Saved") is cleared so it does not stand next to the editor's error.
function startEdit(port: number): void {
  editError.value = "";
  saveStatus.value = { text: "", cls: "" };
  editing.value = port;
}

const sameName = computed<Set<string>>(() => sameNameNodes(net.value.map((it) => it.row)));
const rowKeyOf = (it: NetPortItem): string => netRowKey(it.row, multi.value ? it.slot : undefined);
const refOf = (it: NetPortItem) => ({ slot: it.slot, node: it.row.node, port: it.row.port });

// Local numbers taken by the other rows (a row's own entry does not count).
function takenSet(it: NetPortItem): ReadonlySet<number> | undefined {
  return multi.value ? new Set(takenLocals(store, refOf(it)).keys()) : undefined;
}

// "2080 is already used by <peer> (<net>)" or "" when the number is free.
function usedTextFor(it: NetPortItem): ((local: number) => string) | undefined {
  if (!multi.value) return undefined;
  const taken = takenLocals(store, refOf(it));
  return (local) => {
    const by = taken.get(local);
    const owner = by && net.value.find((o) => o.slot === by.slot && o.row.node === by.node && o.row.port === by.port);
    return by ? localUsedText(local, { peerLabel: owner ? nodeLabel(owner.row) : by.node, net: net.value.find((o) => o.slot === by.slot)?.net ?? by.slot }) : "";
  };
}
</script>

<template>
  <div class="card">
    <h2>{{ PORTS_NET_TITLE }}</h2>
    <div class="table-scroll">
      <table class="data" :aria-label="PORTS_NET_TITLE">
        <thead><tr><th>{{ PORTS_COL_PEER }}</th><th>{{ PORTS_COL_PORT }}</th><th>{{ PORTS_COL_BROWSER }}</th><th>{{ PORTS_COL_PROGRAMS }}</th></tr></thead>
        <tbody id="ports-net-tbody">
          <tr v-if="!net.length"><td colspan="4" class="hint">{{ EMPTY_NET }}</td></tr>
          <NetPortRow
            v-for="it in net" :key="rowKeyOf(it)" :row="it.row" :busy="saving" :slot="it.slot" :net="it.net" :taken="takenSet(it)" :same-name="sameName.has(it.row.node)" :used-text="usedTextFor(it)" :error="openErrors[rowKeyOf(it)]"
            @open="(row: NetPort, local: number | string) => openNetPort(it.slot, row.node, row.port, local)" @close="(row: NetPort) => closeNetPort(it.slot, row.node, row.port)"
          />
        </tbody>
      </table>
    </div>
    <p v-if="sameName.size" class="hint">{{ PORTS_SAME_NAME_HINT }}</p>
    <p v-for="h in PORTS_NET_HINTS" :key="h" class="hint">{{ h }}</p>
  </div>
  <div class="card">
    <h2>{{ PORTS_MINE_TITLE }}</h2>
    <div class="table-scroll">
      <table class="data" :aria-label="PORTS_MINE_TITLE">
        <thead><tr><th>{{ PORTS_COL_PORT }}</th><th>{{ PORTS_COL_NAME }}</th><th>{{ PORTS_COL_AUDIENCE }}</th><th>{{ PORTS_COL_STATUS }}</th><th></th></tr></thead>
        <tbody id="ports-own-tbody">
          <tr v-if="!own.length"><td colspan="5" class="hint">{{ EMPTY_OWN }}</td></tr>
          <OwnPortRow
            v-for="r in own" :key="r.port" :row="r" :entry="entryOf(r.port)" :audience="audienceOfPort(r.port)" :peers="peers" :busy="saving"
            :groups="editing === r.port ? editGroups(r.port) : undefined" :shared="sharedOf(r.port)"
            :editing="editing === r.port" :edit-error="editError" :save="edit"
            @edit="startEdit(r.port)" @cancel="editing = null" @saved="editing = null" @unpublish="unpublish(r.port)"
          />
        </tbody>
      </table>
    </div>
    <PublishForm :busy="saving" :peers="peers" :groups="groups" :publish="publish" />
    <p id="port-add-error" class="field-error-text" role="alert" :hidden="!addError">{{ addError }}</p>
    <p id="ports-status" class="field-status" :class="saveStatus.cls" role="status" :hidden="!saveStatus.text">{{ saveStatus.text }}</p>
    <p class="hint">{{ PORTS_MINE_HINT }}</p>
  </div>
</template>
