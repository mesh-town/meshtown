<script setup lang="ts">
import { computed, inject, ref } from "vue";
import { STORE_KEY } from "../core/store.ts";
import type { Store } from "../core/store.ts";
import { isMulti, netConfig, netName, stateOf } from "../core/slots.ts";
import type { NetPort, PeerInfo, PortProto } from "../core/api-types.ts";
import NetPortRow from "./NetPortRow.vue";
import OwnPortRow from "./OwnPortRow.vue";
import PublishForm from "./PublishForm.vue";
import type { NetPortItem } from "./ports-view.ts";
import {
  EMPTY_NET, EMPTY_OWN, audienceText, configPorts, localUsedText, netPortItems, netRowKey, nodeLabel, ownRows, sameNameNodes, takenLocals,
} from "./ports-view.ts";
import { portKey, protoOf } from "./ports-proto.ts";
import { usePortsActions } from "./use-ports-actions.ts";
import { PORTS_COL_AUDIENCE, PORTS_COL_BROWSER, PORTS_COL_NAME, PORTS_COL_PEER, PORTS_COL_PORT, PORTS_COL_PROGRAMS, PORTS_COL_STATUS, PORTS_MINE_HINT, PORTS_MINE_TITLE, PORTS_NET_HINTS, PORTS_NET_TITLE, PORTS_NO_STORE, PORTS_SAME_NAME_HINT } from "./ports-texts.ts";

// The ports of one network: the members' ports shared with us and the ones we publish there.
// The parent mounts it with `:key="slot"`, so statuses, errors and the open editor do not move between networks.
const props = defineProps<{ slot: string }>();

const injected = inject(STORE_KEY);
if (!injected) throw new Error(PORTS_NO_STORE);
const store: Store = injected;
const { saveStatus, addError, editError, saving, openErrors, publish, edit, unpublish, openNetPort, closeNetPort } = usePortsActions(store, () => props.slot);

const own = computed(() => ownRows(store, props.slot));
const net = computed<NetPortItem[]>(() => netPortItems(store, props.slot));
const peers = computed<PeerInfo[]>(() => stateOf(store, props.slot)?.peers ?? []);

// The key (`portKey`) of the port whose editor is open now (one for the whole table).
const editing = ref<string | null>(null);

function entryOf(port: number, proto: PortProto) {
  return configPorts(netConfig(store, props.slot), "published").find((p) => p.port === port && protoOf(p) === proto);
}

// The result of the previous action ("Saved") is cleared so it does not stand next to the editor's error.
function startEdit(key: string): void {
  editError.value = "";
  saveStatus.value = { text: "", cls: "" };
  editing.value = key;
}

const multi = computed<boolean>(() => isMulti(store));
const sameName = computed<Set<string>>(() => sameNameNodes(net.value.map((it) => it.row)));
const rowKeyOf = (it: NetPortItem): string => netRowKey(it.row);
const refOf = (it: NetPortItem) => ({ slot: it.slot, node: it.row.node, port: it.row.port });
const protoOfItem = (it: NetPortItem): PortProto => protoOf(it.row);

// Local numbers taken by the other rows, in any network (a row's own entry does not count).
function takenSet(it: NetPortItem): ReadonlySet<number> {
  return new Set(takenLocals(store, refOf(it), protoOfItem(it)).keys());
}

function ownerLabel(by: { slot: string; node: string; port: number }, proto: PortProto): string {
  const rows = stateOf(store, by.slot)?.ports?.network ?? [];
  const owner = rows.find((r) => r.node === by.node && r.port === by.port && protoOf(r) === proto);
  return owner ? nodeLabel(owner) : by.node;
}

// "2080 is already used by <peer> (<net>)" or "" when the number is free; the network is named when there are several.
function usedTextFor(it: NetPortItem): (local: number) => string {
  const taken = takenLocals(store, refOf(it), protoOfItem(it));
  return (local) => {
    const by = taken.get(local);
    return by ? localUsedText(local, { peerLabel: ownerLabel(by, protoOfItem(it)), net: multi.value ? netName(store.nets, by.slot) : null }) : "";
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
            v-for="it in net" :key="rowKeyOf(it)" :row="it.row" :busy="saving" :taken="takenSet(it)" :same-name="sameName.has(it.row.node)" :used-text="usedTextFor(it)" :error="openErrors[rowKeyOf(it)]"
            @open="(row: NetPort, local: number | string) => openNetPort(it.slot, row.node, row.port, local, protoOf(row))" @close="(row: NetPort) => closeNetPort(it.slot, row.node, row.port, protoOf(row))"
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
            v-for="r in own" :key="portKey(protoOf(r), r.port)" :row="r" :proto="protoOf(r)" :entry="entryOf(r.port, protoOf(r))" :audience="audienceText(entryOf(r.port, protoOf(r)), peers)" :peers="peers" :busy="saving"
            :editing="editing === portKey(protoOf(r), r.port)" :edit-error="editError" :save="edit"
            @edit="startEdit(portKey(protoOf(r), r.port))" @cancel="editing = null" @saved="editing = null" @unpublish="unpublish(r.port, protoOf(r))"
          />
        </tbody>
      </table>
    </div>
    <PublishForm :busy="saving" :peers="peers" :publish="publish" />
    <p id="port-add-error" class="field-error-text" role="alert" :hidden="!addError">{{ addError }}</p>
    <p id="ports-status" class="field-status" :class="saveStatus.cls" role="status" :hidden="!saveStatus.text">{{ saveStatus.text }}</p>
    <p class="hint">{{ PORTS_MINE_HINT }}</p>
  </div>
</template>
