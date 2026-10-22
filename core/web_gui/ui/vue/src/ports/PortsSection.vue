<script setup lang="ts">
import { computed, inject, ref } from "vue";
import { STORE_KEY } from "../core/store.ts";
import type { Store } from "../core/store.ts";
import type { NetPort, OwnPort, PeerInfo } from "../core/api-types.ts";
import NetPortRow from "./NetPortRow.vue";
import OwnPortRow from "./OwnPortRow.vue";
import PublishForm from "./PublishForm.vue";
import { EMPTY_NET, EMPTY_OWN, audienceText, configPorts, netRowKey, portStatusText } from "./ports-view.ts";
import { usePortsActions } from "./use-ports-actions.ts";
import { PORTS_COL_AUDIENCE, PORTS_COL_NAME, PORTS_COL_ONLINE, PORTS_COL_PEER, PORTS_COL_PORT, PORTS_COL_STATUS, PORTS_MINE_HINT, PORTS_MINE_TITLE, PORTS_NET_HINT, PORTS_NET_TITLE, PORTS_NO_STORE } from "./ports-texts.ts";

const injected = inject(STORE_KEY);
if (!injected) throw new Error(PORTS_NO_STORE);
const store: Store = injected;
const { saveStatus, addError, editError, saving, publish, edit, unpublish, openNetPort, closeNetPort } = usePortsActions(store);

const own = computed<OwnPort[]>(() => (store.state && store.state.ports.published) || []);
const net = computed<NetPort[]>(() => (store.state && store.state.ports.network) || []);

const peers = computed<PeerInfo[]>(() => (store.state && store.state.peers) || []);

// The port whose editor is open now (one for the whole table).
const editing = ref<number | null>(null);

function entryOf(port: number) {
  return configPorts(store.config, "published").find((p) => p.port === port);
}

function audienceOfPort(port: number): string {
  return audienceText(entryOf(port), peers.value);
}

// The result of the previous action ("Saved") is cleared so it does not stand next to the editor's error.
function startEdit(port: number): void {
  editError.value = "";
  saveStatus.value = { text: "", cls: "" };
  editing.value = port;
}
</script>

<template>
  <div class="card">
    <h2>{{ PORTS_MINE_TITLE }}</h2>
    <div class="table-scroll">
      <table class="data" :aria-label="PORTS_MINE_TITLE">
        <thead><tr><th>{{ PORTS_COL_PORT }}</th><th>{{ PORTS_COL_NAME }}</th><th>{{ PORTS_COL_AUDIENCE }}</th><th>{{ PORTS_COL_STATUS }}</th><th></th></tr></thead>
        <tbody id="ports-own-tbody">
          <tr v-if="!own.length"><td colspan="5" class="hint">{{ EMPTY_OWN }}</td></tr>
          <OwnPortRow
            v-for="r in own" :key="r.port" :row="r" :entry="entryOf(r.port)" :audience="audienceOfPort(r.port)" :peers="peers" :busy="saving"
            :editing="editing === r.port" :edit-error="editError" :save="edit"
            @edit="startEdit(r.port)" @cancel="editing = null" @saved="editing = null" @unpublish="unpublish(r.port)"
          />
        </tbody>
      </table>
    </div>
    <PublishForm :busy="saving" :peers="peers" :publish="publish" />
    <p id="port-add-error" class="field-error-text" role="alert" :hidden="!addError">{{ addError }}</p>
    <p id="ports-status" class="field-status" :class="saveStatus.cls" role="status" :hidden="!saveStatus.text">{{ saveStatus.text }}</p>
    <p class="hint">{{ PORTS_MINE_HINT }}</p>
  </div>
  <div class="card">
    <h2>{{ PORTS_NET_TITLE }}</h2>
    <div class="table-scroll">
      <table class="data" :aria-label="PORTS_NET_TITLE">
        <thead><tr><th>{{ PORTS_COL_PEER }}</th><th>{{ PORTS_COL_PORT }}</th><th>{{ PORTS_COL_NAME }}</th><th>{{ PORTS_COL_ONLINE }}</th><th></th></tr></thead>
        <tbody id="ports-net-tbody">
          <tr v-if="!net.length"><td colspan="5" class="hint">{{ EMPTY_NET }}</td></tr>
          <NetPortRow v-for="r in net" :key="netRowKey(r)" :row="r" :busy="saving" @open="(row: NetPort, local: number | string) => openNetPort(row.node, row.port, local)" @close="(row: NetPort) => closeNetPort(row.node, row.port)" />
        </tbody>
      </table>
    </div>
    <p class="hint">{{ PORTS_NET_HINT }}</p>
  </div>
</template>
