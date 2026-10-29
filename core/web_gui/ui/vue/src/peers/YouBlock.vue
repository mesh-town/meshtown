<script setup lang="ts">
// The "Device settings" block of a network tab: this computer's name in that network, the "open sites through
// this computer" switch (not in the phone app) and the way out of the network. The name is saved on
// Enter and when leaving the field; the parent acts on `leave`.
import { computed, inject, ref, watch } from "vue";
import { fieldErrorText, saveSlotFields } from "../core/save-config.ts";
import { netConfig, netName, stateOf, visibleSlots } from "../core/slots.ts";
import { STORE_KEY } from "../core/store.ts";
import type { ConfigDocument } from "../core/api-types.ts";
import type { FieldStatus } from "../home/home-actions.ts";
import { HOME_SAVED } from "../home/home-flow-texts.ts";
import { inAppPage } from "../home/browser-setup.ts";
import { exitServerHint } from "../home/home-words.ts";
import { configFieldAccess, nodeNameFieldValue } from "../home/home-view.ts";
import { getPath } from "../settings/settings-fields.ts";
import { MENU_DELETE_NETWORK } from "../members/members-managed-texts.ts";
import { canDeleteNet } from "../members/managed-ctx.ts";
import ExitIfaceField from "./ExitIfaceField.vue";
import { PEERS_NO_STORE } from "./peers-texts.ts";
import { YOU_LEAVE_BTN, YOU_NAME_PLACEHOLDER, youExitLabel, youExitOff, youLeaveHint, youNameHint, youNameLabel, youSameName, youTitle } from "./you-texts.ts";

const props = defineProps<{ slot: string }>();
const emit = defineEmits<{ (e: "leave"): void; (e: "delete"): void }>();
const store = inject(STORE_KEY);
if (!store) throw new Error(PEERS_NO_STORE);

const inApp = inAppPage();
const NO_STATUS: FieldStatus = { text: "", cls: "" };
const net = computed(() => netName(store.nets, props.slot));
const visible = computed(() => visibleSlots(store.nets, store.slots));
const first = computed(() => visible.value[0]);
const metaOf = (id: string) => (id === "main" ? store.configMeta : (store.slots[id]?.configMeta ?? null));
const nameOf = (id: string): string => nodeNameFieldValue(netConfig(store, id), stateOf(store, id));
const canDelete = computed(() => canDeleteNet(stateOf(store, props.slot)?.network));
const saved = computed(() => nameOf(props.slot));
const nameAccess = computed(() => configFieldAccess(metaOf(props.slot), "node.name"));
const exitAccess = computed(() => configFieldAccess(metaOf(props.slot), "exit.enabled"));
const exitOn = computed(() => !!getPath(netConfig(store, props.slot) as ConfigDocument | null, "exit.enabled"));
const sameFrom = computed(() => (props.slot !== first.value ? nameOf(first.value) : ""));

const draft = ref(saved.value);
const focused = ref(false);
const nameStatus = ref<FieldStatus>(NO_STATUS);
const exitStatus = ref<FieldStatus>(NO_STATUS);
const input = ref<HTMLInputElement | null>(null);
watch([saved, () => props.slot], () => {
  if (!focused.value) draft.value = saved.value;
});

async function save(path: string, value: string | boolean, status: { value: FieldStatus }): Promise<boolean> {
  const changes = { [path]: value };
  const res = await saveSlotFields(store!, props.slot, changes);
  status.value = res.ok ? { text: HOME_SAVED, cls: "saved" } : { text: fieldErrorText(res.error, changes), cls: "field-error-text" };
  return res.ok;
}

async function saveName(): Promise<void> {
  const value = draft.value.trim();
  draft.value = value;
  if (value !== saved.value) await save("node.name", value, nameStatus);
}
const onBlur = (): Promise<void> => {
  focused.value = false;
  return saveName();
};
const onEnter = (ev: KeyboardEvent): void => {
  saveName();
  (ev.target as HTMLInputElement).blur();
};
async function useSame(): Promise<void> {
  draft.value = sameFrom.value;
  await saveName();
}
const onToggle = (ev: Event): Promise<boolean> => save("exit.enabled", (ev.target as HTMLInputElement).checked, exitStatus);
defineExpose({ focusName: (): void => input.value?.focus() });
</script>

<template>
  <div class="card" id="you-block" role="region" aria-labelledby="you-title">
    <h2 id="you-title">{{ youTitle(net) }}</h2>
    <label class="field-label" for="you-name">{{ youNameLabel(net) }}</label>
    <input id="you-name" ref="input" v-model="draft" type="text" class="input" :placeholder="YOU_NAME_PLACEHOLDER" :disabled="nameAccess.readOnly" @focus="focused = true" @input="nameStatus = NO_STATUS" @blur="onBlur" @keydown.enter.prevent="onEnter">
    <p v-if="sameFrom" class="hint"><a href="#" id="you-same-name" @click.prevent="useSame">{{ youSameName(netName(store.nets, first)) }}</a></p>
    <p class="hint">{{ youNameHint(net) }}</p>
    <p class="setting-desc cli-note" id="you-name-cli-note" :hidden="!nameAccess.cliNote">{{ nameAccess.cliNote }}</p>
    <p class="field-status" :class="nameStatus.cls" id="you-name-status" role="status" :hidden="!nameStatus.text">{{ nameStatus.text }}</p>
    <template v-if="!inApp">
    <label class="toggle-inline home-exit-toggle">
      <span class="toggle-inline-text">{{ youExitLabel(net) }}</span>
      <span class="toggle"><input id="you-exit-toggle" type="checkbox" :checked="exitOn" :disabled="exitAccess.readOnly" @change="onToggle"><span class="toggle-track"></span></span>
    </label>
    <p class="hint" id="you-exit-hint">{{ exitOn ? exitServerHint(true, net) : youExitOff(net) }}</p>
    <p class="setting-desc cli-note" id="you-exit-cli-note" :hidden="!exitAccess.cliNote">{{ exitAccess.cliNote }}</p>
    <p class="field-status" :class="exitStatus.cls" id="you-exit-status" role="status" :hidden="!exitStatus.text">{{ exitStatus.text }}</p>
    <ExitIfaceField v-if="exitOn" :slot="slot" />
    </template>
    <p class="hint">{{ youLeaveHint(net) }}</p>
    <div class="btn-row">
      <button type="button" class="btn btn-danger" id="you-leave" @click="emit('leave')">{{ YOU_LEAVE_BTN }}</button>
      <button v-if="canDelete" type="button" class="btn btn-danger" id="you-delete-net-btn" @click="emit('delete')">{{ MENU_DELETE_NETWORK }}</button>
    </div>
  </div>
</template>
