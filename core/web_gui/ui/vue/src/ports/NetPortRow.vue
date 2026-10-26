<script setup lang="ts">
import { computed, ref } from "vue";
import type { NetPort } from "../core/api-types.ts";
import { writeClipboard } from "../common/clipboard.ts";
import { defaultLocalPort, isBrowserBlockedPort, netRowKey, nodeLabel, openErrorText, portStatusText } from "./ports-view.ts";
import { lastErrorText } from "./ports-proto.ts";
import {
  PORTS_CHANGE_NUMBER, PORTS_CLOSE, PORTS_COPIED, PORTS_COPY, PORTS_LOCAL_PORT_LABEL, PORTS_LOCAL_PORT_TITLE, PORTS_NOT_FOR_BROWSER, PORTS_NOT_FOR_BROWSER_TITLE, PORTS_OFFLINE, PORTS_ONLINE, PORTS_OPEN,
  PORTS_OPEN_TITLE, PORTS_OPENED, PORTS_PROTO_UDP, PORTS_SAME_NAME_TITLE, portsCopyAddressLabel, portsLocalAddress,
} from "./ports-texts.ts";

// `taken` holds the local numbers already opened in any network, `usedText(local)` says by whom a number is used ("" — free), `error` is the refusal of "Open"/"Close".
const props = withDefaults(defineProps<{
  row: NetPort;
  busy?: boolean;
  taken?: ReadonlySet<number>;
  usedText?: (local: number) => string;
  error?: string;
  sameName?: boolean;
}>(), { busy: false, taken: undefined, usedText: undefined, error: "", sameName: false });
const emit = defineEmits<{
  (e: "open", row: NetPort, local: number | string): void;
  (e: "close", row: NetPort): void;
}>();

// The local number entered stays in the row: a poll does not erase the draft, but a row that disappeared and came back
// starts over with `defaultLocalPort(row.port)`.
const draft = ref<number | string>(defaultLocalPort(props.row.port, props.taken));
const rowKey = computed(() => netRowKey(props.row));
const label = computed(() => nodeLabel(props.row));
// The hint under "Open": the typed number, or the usual one (member's port plus 2000) that the field skipped, is used by another row.
const draftUsed = computed(() => {
  const used = props.usedText;
  if (!used) return "";
  const skipped = Number(draft.value) === defaultLocalPort(props.row.port, props.taken) ? used(defaultLocalPort(props.row.port)) : "";
  return used(Number(draft.value)) || skipped;
});
const openedUsed = computed(() => (props.usedText && props.row.opened ? props.usedText(props.row.opened.local) : ""));

// The text just copied from this row ("" — nothing); the button says "Copied" for a moment.
const copied = ref("");
const COPIED_MS = 2000;
const blocked = computed(() => props.row.proto !== "udp" && isBrowserBlockedPort(props.row.port));
const online = computed(() => props.row.online);
const udp = computed(() => props.row.proto === "udp");
const lastError = computed(() => lastErrorText(props.row));

async function copy(text: string): Promise<void> {
  if (!(await writeClipboard({ clipboard: navigator.clipboard }, text))) return;
  copied.value = text;
  setTimeout(() => { if (copied.value === text) copied.value = ""; }, COPIED_MS);
}

function open(): void {
  emit("open", props.row, draft.value);
  draft.value = defaultLocalPort(props.row.port, props.taken);
  changing.value = false;
}

// "Change number" swaps the opened address for the number field; the field starts from the current number.
const changing = ref(false);
function startChange(): void {
  if (props.row.opened) draft.value = props.row.opened.local;
  changing.value = true;
}
</script>

<template>
  <tr :data-port-row="rowKey">
    <td :title="row.node">{{ label }} <span class="badge" :class="{ 'badge-ok': online }">{{ online ? PORTS_ONLINE : PORTS_OFFLINE }}</span></td>
    <td><span class="mono">{{ row.port }}</span><template v-if="udp"> <span class="badge badge-grey" data-port-proto="udp">{{ PORTS_PROTO_UDP }}</span></template><div v-if="row.name">{{ row.name }}</div></td>
    <td>
      <div class="addrcell">
        <span v-if="blocked" class="muted" :title="PORTS_NOT_FOR_BROWSER_TITLE">{{ PORTS_NOT_FOR_BROWSER }}</span>
        <template v-else-if="!udp && row.web_url">
          <a class="mono" :href="row.web_url" target="_blank" rel="noopener" :title="sameName ? PORTS_SAME_NAME_TITLE : undefined">{{ row.web_url }}</a>
          <button type="button" class="btn btn-sm" :aria-label="portsCopyAddressLabel(row.web_url)" @click="copy(row.web_url)">{{ copied === row.web_url ? PORTS_COPIED : PORTS_COPY }}</button>
        </template>
      </div>
    </td>
    <td>
      <div class="opencell">
        <div v-if="row.opened && row.opened.state === 'bind_failed'" class="field-error-text">{{ portStatusText(row) }}</div>
        <template v-if="row.opened && row.opened.state === 'bind_failed'">
          <div v-if="row.opened.error" class="field-error-text">{{ openErrorText(row.opened.error) }}</div>
          <input v-model="draft" type="number" class="input input-port" min="1" max="65535" :data-port-local="rowKey" :title="PORTS_LOCAL_PORT_TITLE">
          <button type="button" class="btn btn-sm" :data-port-open="rowKey" :title="PORTS_OPEN_TITLE" :disabled="!row.published || busy" @click="open">{{ PORTS_OPEN }}</button>
          <button type="button" class="btn btn-sm" :data-port-close="rowKey" :disabled="busy" @click="emit('close', row)">{{ PORTS_CLOSE }}</button>
        </template>
        <template v-else-if="row.opened && changing">
          <input v-model="draft" type="number" class="input input-port" min="1" max="65535" :data-port-local="rowKey" :aria-label="PORTS_LOCAL_PORT_LABEL" :title="PORTS_LOCAL_PORT_TITLE">
          <button type="button" class="btn btn-sm" :data-port-open="rowKey" :title="PORTS_OPEN_TITLE" :disabled="!row.published || busy" @click="open">{{ PORTS_OPEN }}</button>
          <div v-if="lastError" class="field-error-text" :data-port-last-error="rowKey">{{ lastError }}</div>
        </template>
        <template v-else-if="row.opened">
          <span class="mono">{{ portsLocalAddress(row.opened.local) }}</span>
          <button type="button" class="btn btn-sm" :aria-label="portsCopyAddressLabel(portsLocalAddress(row.opened.local))" @click="copy(portsLocalAddress(row.opened.local))">{{ copied === portsLocalAddress(row.opened.local) ? PORTS_COPIED : PORTS_COPY }}</button>
          <span class="badge" :class="{ 'badge-ok': !lastError }">{{ PORTS_OPENED }}</span>
          <button type="button" class="btn btn-sm" :data-port-close="rowKey" :disabled="busy" @click="emit('close', row)">{{ PORTS_CLOSE }}</button>
          <button type="button" class="linkbtn" :data-port-change="rowKey" :disabled="busy" @click="startChange">{{ PORTS_CHANGE_NUMBER }}</button>
          <div v-if="row.opened.error" class="field-error-text">{{ openErrorText(row.opened.error) }}</div>
          <div v-if="openedUsed" class="hint">{{ openedUsed }}</div>
          <div v-if="lastError" class="field-error-text" :data-port-last-error="rowKey">{{ lastError }}</div>
        </template>
        <template v-else>
          <input v-model="draft" type="number" class="input input-port" min="1" max="65535" :data-port-local="rowKey" :title="PORTS_LOCAL_PORT_TITLE">
          <button type="button" class="btn btn-sm" :data-port-open="rowKey" :title="PORTS_OPEN_TITLE" :disabled="!row.published || busy" @click="open">{{ PORTS_OPEN }}</button>
          <div v-if="draftUsed" class="hint">{{ draftUsed }}</div>
        </template>
        <div v-if="error" class="field-error-text" :data-port-error="rowKey">{{ error }}</div>
      </div>
    </td>
  </tr>
</template>
