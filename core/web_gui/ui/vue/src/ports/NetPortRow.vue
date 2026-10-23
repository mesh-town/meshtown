<script setup lang="ts">
import { computed, ref } from "vue";
import type { NetPort } from "../core/api-types.ts";
import { defaultLocalPort, netRowKey, nodeLabel, openErrorText, peerNetLabel, portStatusText } from "./ports-view.ts";
import { PORTS_CLOSE, PORTS_LOCAL_PORT_TITLE, PORTS_OPEN, PORTS_OPENED } from "./ports-texts.ts";

// With several networks: `slot` and `net` (the network name) tell the rows of different networks apart, `taken` holds the local
// numbers already opened anywhere, `usedText(local)` says by whom a number is used ("" — free), `error` is the refusal of "Open"/"Close".
const props = withDefaults(defineProps<{
  row: NetPort;
  busy?: boolean;
  slot?: string;
  net?: string | null;
  taken?: ReadonlySet<number>;
  usedText?: (local: number) => string;
  error?: string;
}>(), { busy: false, slot: undefined, net: null, taken: undefined, usedText: undefined, error: "" });
const emit = defineEmits<{
  (e: "open", row: NetPort, local: number | string): void;
  (e: "close", row: NetPort): void;
}>();

// The local number entered stays in the row: a poll does not erase the draft, but a row that disappeared and came back
// starts over with `defaultLocalPort(row.port)`.
const draft = ref<number | string>(defaultLocalPort(props.row.port, props.taken));
const rowKey = computed(() => netRowKey(props.row, props.net ? props.slot : undefined));
const label = computed(() => peerNetLabel(nodeLabel(props.row), props.net));
// The hint under "Open": the typed number, or the usual one (member's port plus 2000) that the field skipped, is used by another row.
const draftUsed = computed(() => {
  const used = props.usedText;
  if (!used) return "";
  const skipped = Number(draft.value) === defaultLocalPort(props.row.port, props.taken) ? used(defaultLocalPort(props.row.port)) : "";
  return used(Number(draft.value)) || skipped;
});
const openedUsed = computed(() => (props.usedText && props.row.opened ? props.usedText(props.row.opened.local) : ""));

function open(): void {
  emit("open", props.row, draft.value);
  draft.value = defaultLocalPort(props.row.port, props.taken);
}
</script>

<template>
  <tr :data-port-row="rowKey">
    <td :title="row.node">{{ label }}</td>
    <td class="mono">{{ row.port }}</td>
    <td>{{ row.name }}</td>
    <td>{{ portStatusText(row) }}</td>
    <td>
      <template v-if="row.opened && row.opened.state === 'bind_failed'">
        <div v-if="row.opened.error" class="field-error-text">{{ openErrorText(row.opened.error) }}</div>
        <input v-model="draft" type="number" class="input input-port" min="1" max="65535" :data-port-local="rowKey" :title="PORTS_LOCAL_PORT_TITLE">
        <button type="button" class="btn btn-sm" :data-port-open="rowKey" :disabled="!row.published || busy" @click="open">{{ PORTS_OPEN }}</button>
        <button type="button" class="btn btn-sm" :data-port-close="rowKey" :disabled="busy" @click="emit('close', row)">{{ PORTS_CLOSE }}</button>
      </template>
      <template v-else-if="row.opened">
        <span class="mono">127.0.0.1:{{ row.opened.local }}</span> <span class="badge badge-ok">{{ PORTS_OPENED }}</span>
        <button type="button" class="btn btn-sm" :data-port-close="rowKey" :disabled="busy" @click="emit('close', row)">{{ PORTS_CLOSE }}</button>
        <div v-if="row.opened.error" class="field-error-text">{{ openErrorText(row.opened.error) }}</div>
        <div v-if="openedUsed" class="hint">{{ openedUsed }}</div>
      </template>
      <template v-else>
        <input v-model="draft" type="number" class="input input-port" min="1" max="65535" :data-port-local="rowKey" :title="PORTS_LOCAL_PORT_TITLE">
        <button type="button" class="btn btn-sm" :data-port-open="rowKey" :disabled="!row.published || busy" @click="open">{{ PORTS_OPEN }}</button>
        <div v-if="draftUsed" class="hint">{{ draftUsed }}</div>
      </template>
      <div v-if="error" class="field-error-text" :data-port-error="rowKey">{{ error }}</div>
    </td>
  </tr>
</template>
