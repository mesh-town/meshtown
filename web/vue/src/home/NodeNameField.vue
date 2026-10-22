<script setup lang="ts">
// The "Name of this computer" field: saved on Enter and when leaving the field. Sits on "Home" and in
// "Settings"; `idBase` separates the ids because all the page sections are mounted at once.
import { inject } from "vue";
import { STORE_KEY } from "../core/store.ts";
import type { FieldStatus } from "./home-actions.ts";
import { HOME_NAME_HINT, HOME_NAME_LABEL, HOME_NAME_LABEL_PHONE, HOME_NAME_PLACEHOLDER, HOME_NAME_SAVE } from "./home-card-texts.ts";
import { inAppPage } from "./browser-setup.ts";
import { useHomeContext } from "./use-home.ts";

withDefaults(defineProps<{ idBase?: string; withSave?: boolean }>(), { idBase: "field-node-name", withSave: false });
const { form, nameView, ...h } = useHomeContext(inject(STORE_KEY)!);
const nameLabel = inAppPage() ? HOME_NAME_LABEL_PHONE : HOME_NAME_LABEL;
const NO_STATUS: FieldStatus = { text: "", cls: "" };

const onFocus = () => { form.name.focused = true; };
const onInput = () => { form.name.status = NO_STATUS; };
const onBlur = () => { form.name.focused = false; return h.saveName(); };
const onEnter = (ev: KeyboardEvent): void => { h.saveName(); (ev.target as HTMLInputElement).blur(); };
</script>

<template>
  <label class="field-label" :for="idBase">{{ nameLabel }}</label>
  <div :class="{ 'name-row': withSave }">
    <input type="text" class="input" :id="idBase" v-model="form.name.draft" :placeholder="HOME_NAME_PLACEHOLDER" :disabled="nameView.readOnly" @focus="onFocus" @input="onInput" @blur="onBlur" @keydown.enter.prevent="onEnter">
    <button v-if="withSave" type="button" class="btn" :disabled="nameView.readOnly" @click="h.saveName()">{{ HOME_NAME_SAVE }}</button>
  </div>
  <p class="hint">{{ HOME_NAME_HINT }}</p>
  <p class="setting-desc cli-note" :id="idBase + '-cli-note'" :hidden="!nameView.cliNote">{{ nameView.cliNote }}</p>
  <p class="field-status" :class="form.name.status.cls" :id="idBase + '-status'" role="status" :hidden="!form.name.status.text">{{ form.name.status.text }}</p>
</template>
