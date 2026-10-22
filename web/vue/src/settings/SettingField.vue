<script setup lang="ts">
// Field row: the classic `fieldRow` markup. The value and rules come from `view` (`fieldView`).
import { selectOptions } from "./settings-fields.ts";
import { SETTINGS_RESTART_BADGE } from "./settings-texts.ts";
import type { Field, RawValue } from "./settings-fields.ts";
import type { FieldView } from "./settings-view.ts";

const props = defineProps<{ field: Field; view: FieldView }>();
const emit = defineEmits<{ (e: "edit", path: string, raw: RawValue): void }>();

const isBool = (v: unknown): v is boolean => v === true || v === false;
const readText = (v: unknown): string => (v === null || v === undefined || v === "" ? "—" : String(v));

function onInput(ev: Event): void {
  emit("edit", props.field.path, (ev.target as HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement).value);
}
function onChecked(ev: Event): void {
  emit("edit", props.field.path, (ev.target as HTMLInputElement).checked);
}
</script>

<template>
  <div class="setting-row">
    <div class="setting-text">
      <div class="setting-title">{{ field.title }} <span v-if="view.restartBadge" class="mini-badge mini-badge-restart">{{ SETTINGS_RESTART_BADGE }}</span></div>
      <div v-if="field.desc" class="setting-desc">{{ field.desc }}</div>
      <div v-if="view.cliNote" class="setting-desc cli-note" :data-note-for="field.path" :data-readonly="view.readOnly ? '' : null">{{ view.cliNote }}</div>
      <div v-else-if="view.readonlyNote || view.disabledReason" class="setting-desc readonly-note">{{ view.readonlyNote || view.disabledReason }}</div>
      <div class="field-error-text" :id="view.id + '-error'" :hidden="!view.error">{{ view.error }}</div>
    </div>
    <div class="setting-control">
      <span v-if="view.readOnly && isBool(view.value)" class="toggle"><input type="checkbox" :id="view.id" :aria-label="field.title" :checked="view.value === true" disabled><span class="toggle-track"></span></span>
      <span v-else-if="view.readOnly" class="value-box">{{ readText(view.value) }}</span>
      <span v-else-if="field.kind === 'toggle'" class="toggle"><input type="checkbox" :id="view.id" :aria-label="field.title" :checked="view.value === true" :disabled="view.disabled" @change="onChecked"><span class="toggle-track"></span></span>
      <select v-else-if="field.kind === 'select'" :id="view.id" :class="{ 'field-error': view.error }" :aria-label="field.title" :aria-describedby="view.id + '-error'" :value="String(view.value)" :disabled="view.disabled" @change="onInput">
        <option v-for="o in selectOptions(field, view.value)" :key="o[0]" :value="o[0]">{{ o[1] }}</option>
      </select>
      <textarea v-else-if="field.kind === 'lines'" :id="view.id" class="mono lines-input" :class="{ 'field-error': view.error }" :aria-label="field.title" :aria-describedby="view.id + '-error'" :value="String(view.value)" :disabled="view.disabled" @input="onInput"></textarea>
      <input v-else type="text" class="input" :class="{ 'field-error': view.error }" :id="view.id" :aria-label="field.title" :aria-describedby="view.id + '-error'" :placeholder="field.placeholder || ''" :value="String(view.value)" :disabled="view.disabled" @input="onInput">
    </div>
  </div>
</template>
