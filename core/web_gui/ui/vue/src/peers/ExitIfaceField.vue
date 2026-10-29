<script setup lang="ts">
// "Send exit traffic through": which interface the exit connections of this computer are bound to.
import { computed, inject, ref } from "vue";
import { fieldErrorText, saveSlotFields } from "../core/save-config.ts";
import { netConfig, stateOf } from "../core/slots.ts";
import { STORE_KEY } from "../core/store.ts";
import type { ConfigDocument } from "../core/api-types.ts";
import type { FieldStatus } from "../home/home-actions.ts";
import { HOME_SAVED } from "../home/home-flow-texts.ts";
import { configFieldAccess } from "../home/home-view.ts";
import { getPath } from "../settings/settings-fields.ts";
import { forDevice } from "../common/device.ts";
import { PEERS_NO_STORE } from "./peers-texts.ts";
import { EXIT_IFACE_LABEL, exitIfaceHint } from "./exit-iface-texts.ts";
import { ifaceOptions, ifaceStateText } from "./exit-iface-view.ts";

const props = defineProps<{ slot: string }>();
const store = inject(STORE_KEY);
if (!store) throw new Error(PEERS_NO_STORE);

const NO_STATUS: FieldStatus = { text: "", cls: "" };
const meta = computed(() => (props.slot === "main" ? store.configMeta : (store.slots[props.slot]?.configMeta ?? null)));
const access = computed(() => configFieldAccess(meta.value, "exit.interface"));
const current = computed(() => {
  const v = getPath(netConfig(store, props.slot) as ConfigDocument | null, "exit.interface");
  return typeof v === "string" && v ? v : null;
});
const state = computed(() => stateOf(store, props.slot));
const options = computed(() => ifaceOptions(state.value?.node?.networks ?? [], current.value));
const stateText = computed(() => ifaceStateText(state.value?.exit?.interface));
const status = ref<FieldStatus>(NO_STATUS);

async function onChange(ev: Event): Promise<void> {
  const value = (ev.target as HTMLSelectElement).value;
  const changes = { "exit.interface": value || null };
  const res = await saveSlotFields(store!, props.slot, changes);
  status.value = res.ok ? { text: HOME_SAVED, cls: "saved" } : { text: fieldErrorText(res.error, changes), cls: "field-error-text" };
}
</script>

<template>
  <label class="field-label" for="you-exit-iface">{{ forDevice(EXIT_IFACE_LABEL) }}</label>
  <select id="you-exit-iface" class="input" :value="current ?? ''" :disabled="access.readOnly" @change="onChange">
    <option v-for="[value, label] in options" :key="value" :value="value">{{ label }}</option>
  </select>
  <p class="hint">{{ exitIfaceHint() }}</p>
  <p class="hint warn" id="you-exit-iface-state" role="status" :hidden="!stateText">{{ stateText }}</p>
  <p class="field-status" :class="status.cls" id="you-exit-iface-status" role="status" :hidden="!status.text">{{ status.text }}</p>
</template>
