<script setup lang="ts">
// The load of this computer as an exit: open connections, the overload warning, who was turned away and the
// "Connection limit" field. The limit is a process-wide setting kept in the config of the main network, so it is
// saved there from the tab of any network.
import { computed, inject, ref, watch } from "vue";
import { fieldErrorText, saveConfigFields } from "../core/save-config.ts";
import { stateOf } from "../core/slots.ts";
import { STORE_KEY } from "../core/store.ts";
import type { ConfigDocument } from "../core/api-types.ts";
import type { FieldStatus } from "../home/home-actions.ts";
import { HOME_SAVED } from "../home/home-flow-texts.ts";
import { configFieldAccess } from "../home/home-view.ts";
import { getPath } from "../settings/settings-fields.ts";
import { PEERS_NO_STORE } from "./peers-texts.ts";
import { EXIT_LIMIT_INVALID, EXIT_LIMIT_LABEL, EXIT_TURNED_AWAY_TITLE, exitLimitHint, exitStreamsLine, exitTurnedAwayRows, exitWarningText, parseExitLimit } from "./exit-load.ts";

const props = defineProps<{ slot: string }>();
const store = inject(STORE_KEY);
if (!store) throw new Error(PEERS_NO_STORE);

const NO_STATUS: FieldStatus = { text: "", cls: "" };
const PATH = "exit.max_streams";
const state = computed(() => stateOf(store, props.slot));
const load = computed(() => state.value?.exit?.load ?? null);
const access = computed(() => configFieldAccess(store.configMeta, PATH));
const saved = computed(() => {
  const v = getPath(store.config as ConfigDocument | null, PATH);
  return typeof v === "number" ? String(v) : "0";
});
const warning = computed(() => (load.value ? exitWarningText(load.value.overloaded, load.value.limit) : null));
const rows = computed(() => (load.value ? exitTurnedAwayRows(load.value, state.value?.peers) : []));
const refusedAny = computed(() => (load.value?.refused ?? 0) > 0 || (load.value?.elsewhere_refused ?? 0) > 0);

const draft = ref(saved.value);
const focused = ref(false);
const status = ref<FieldStatus>(NO_STATUS);
watch(saved, () => {
  if (!focused.value) draft.value = saved.value;
});

async function commit(): Promise<void> {
  const n = parseExitLimit(draft.value);
  if (n === null) {
    status.value = { text: EXIT_LIMIT_INVALID, cls: "field-error-text" };
    return;
  }
  draft.value = String(n);
  if (draft.value === saved.value) return;
  const changes = { [PATH]: n };
  const res = await saveConfigFields(store!, changes);
  status.value = res.ok ? { text: HOME_SAVED, cls: "saved" } : { text: fieldErrorText(res.error, changes), cls: "field-error-text" };
}
const onBlur = (): Promise<void> => {
  focused.value = false;
  return commit();
};
// Enter only leaves the field: the blur handler saves, so the value is sent once.
const onEnter = (ev: KeyboardEvent): void => (ev.target as HTMLInputElement).blur();
</script>

<template>
  <div v-if="load" id="exit-load">
    <p class="hint" id="exit-load-streams">{{ exitStreamsLine(load) }}</p>
    <p v-if="warning" class="hint badge-busy exit-load-warning" id="exit-load-warning" role="status">{{ warning }}</p>
    <div v-if="refusedAny" id="exit-load-table">
      <p class="field-label">{{ EXIT_TURNED_AWAY_TITLE }}</p>
      <p v-for="(row, i) in rows" :key="i" class="hint exit-load-row">{{ row }}</p>
    </div>
    <label class="field-label" for="exit-load-limit">{{ EXIT_LIMIT_LABEL }}</label>
    <input id="exit-load-limit" v-model="draft" type="text" inputmode="numeric" class="input" :disabled="access.readOnly" @focus="focused = true" @input="status = NO_STATUS" @blur="onBlur" @keydown.enter.prevent="onEnter">
    <p class="hint">{{ exitLimitHint(load.limit_auto) }}</p>
    <p class="setting-desc cli-note" id="exit-load-cli-note" :hidden="!access.cliNote">{{ access.cliNote }}</p>
    <p class="field-status" :class="status.cls" id="exit-load-status" role="status" :hidden="!status.text">{{ status.text }}</p>
  </div>
</template>
