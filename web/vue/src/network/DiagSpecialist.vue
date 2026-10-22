<script setup lang="ts">
import { computed, inject } from "vue";
import { STORE_KEY } from "../core/store.ts";
import NetKeyCard from "./NetKeyCard.vue";
import {
  DIAG_ABOUT_TITLE, DIAG_ADD_BTN, DIAG_CONNECTION_TITLE, DIAG_COPY_BTN, DIAG_MANUAL_HINT, DIAG_MANUAL_TITLE, DIAG_PEER_ADDRESSES_LABEL,
  DIAG_PEER_ADDRESSES_PLACEHOLDER, DIAG_SELF_ADDRESSES_LABEL, DIAG_SPECIALIST_TITLE, DIAG_STORE_MISSING,
} from "./diag-texts.ts";
import type { Diagnostics } from "./use-diagnostics.ts";

const props = defineProps<{ diag: Diagnostics }>();
const store = inject(STORE_KEY);
if (!store) throw new Error(DIAG_STORE_MISSING);
const { form, transport, about, copySelfAddresses, addPeerAddresses } = props.diag;
const hasNetwork = computed(() => !!(store.state && store.state.network));
const selfAddresses = computed(() => ((store.state && store.state.transport && store.state.transport.self_addresses) || []).join("\n"));
const onToggle = (ev: Event): void => { form.specOpen = (ev.target as HTMLDetailsElement).open; };
</script>

<template>
  <details class="card" id="diag-specialist" :open="form.specOpen" @toggle="onToggle">
    <summary class="setting-title">{{ DIAG_SPECIALIST_TITLE }}</summary>
    <template v-if="hasNetwork">
      <h3 id="tp-table-title">{{ DIAG_CONNECTION_TITLE }}</h3>
      <table class="kv-table" id="tp-table" aria-labelledby="tp-table-title">
        <tbody>
          <tr v-for="row in transport" :key="row.label">
            <th scope="row">{{ row.label }}</th>
            <td :class="{ mono: row.mono }">{{ row.value }}</td>
          </tr>
        </tbody>
      </table>
      <NetKeyCard :diag="props.diag" />
      <div class="card">
        <h2>{{ DIAG_MANUAL_TITLE }}</h2>
        <div class="two-col">
          <div>
            <textarea class="mono" id="self-addresses" readonly :aria-label="DIAG_SELF_ADDRESSES_LABEL" :value="selfAddresses"></textarea>
            <button type="button" class="btn" id="self-addr-copy-btn" @click="copySelfAddresses">{{ DIAG_COPY_BTN }}</button>
            <p v-if="form.copyStatus" class="hint" role="status">{{ form.copyStatus }}</p>
          </div>
          <div>
            <textarea id="peer-addr-input" v-model="form.peerAddr.draft" :aria-label="DIAG_PEER_ADDRESSES_LABEL" :placeholder="DIAG_PEER_ADDRESSES_PLACEHOLDER"></textarea>
            <button type="button" class="btn btn-primary" id="peer-addr-add-btn" :disabled="form.peerAddr.busy" @click="addPeerAddresses">{{ DIAG_ADD_BTN }}</button>
            <p v-if="form.peerAddr.result" class="hint" id="peer-addr-result" role="status">{{ form.peerAddr.result }}</p>
          </div>
        </div>
        <p class="hint">{{ DIAG_MANUAL_HINT }}</p>
      </div>
    </template>
    <div class="card" id="about">
      <h2>{{ DIAG_ABOUT_TITLE }}</h2>
      <p v-for="line in about" :key="line" class="hint">{{ line }}</p>
    </div>
  </details>
</template>
