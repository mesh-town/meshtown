<script setup lang="ts">
// The join card: one component for two places — "Home" without a network (the form is chosen by the setup's panel
// `#home-start-choice`) and "Settings" with a network (`joinPlace`). `join` is a ready `useJoin` from the parent (the setup drives the form itself), otherwise its own.
import { computed, inject } from "vue";
import { STORE_KEY } from "../core/store.ts";
import { useJoin } from "./use-join.ts";
import { inviteWaitText } from "./join-wait.ts";
import type { JoinView } from "./use-join.ts";
import {
  HOME_CREATE_BTN, HOME_CREATE_BUSY, HOME_CREATE_ERROR_ARIA, HOME_CREATE_LABEL, HOME_CREATE_PLACEHOLDER, HOME_CREATE_TOGGLE,
  HOME_JOIN_BUSY, HOME_JOIN_ERROR_ARIA, HOME_JOIN_FIELD_LABEL, HOME_JOIN_PASTE_HINT, HOME_JOIN_PLACEHOLDER, HOME_JOIN_RESTART_HINT,
} from "./home-card-texts.ts";

const props = defineProps<{ join?: JoinView | null; quiet?: boolean }>();
const store = inject(STORE_KEY)!;
const j = props.join || useJoin(store);
const { form, hasNet, showJoin, showCreate, title, buttonText, errorParts } = j;
// `quiet` — the setup has already shown the join progress in its own block, so we don't duplicate the line under the form.
const waitText = computed(() => (props.quiet ? "" : inviteWaitText(store.state?.node?.invite_join)));
</script>

<template>
  <div id="join-card" role="region" aria-labelledby="join-card-title">
    <div class="step-head"><h2 id="join-card-title">{{ title }}</h2></div>
    <div id="join-box" :hidden="!showJoin">
    <label class="field-label" for="join-key-input">{{ HOME_JOIN_FIELD_LABEL }}</label>
    <input type="text" class="input mono input-wide" :class="{ 'field-error': form.inviteError }" id="join-key-input" v-model="form.inviteInput" autocomplete="off" spellcheck="false" aria-describedby="join-key-error net-restart-hint" :placeholder="HOME_JOIN_PLACEHOLDER">
    <p class="field-error-text" id="join-key-error" role="alert" :aria-label="HOME_JOIN_ERROR_ARIA" :hidden="!form.inviteError"><template v-for="(part, i) in errorParts" :key="i"><mark v-if="part.highlighted" class="key-error-word">{{ part.text }}</mark><template v-else>{{ part.text }}</template></template></p>
    <p class="hint" id="net-restart-hint" :hidden="!hasNet">{{ HOME_JOIN_RESTART_HINT }}</p>
    <p class="hint">{{ HOME_JOIN_PASTE_HINT }}</p>
    <div class="btn-row">
      <button type="button" class="btn btn-primary" id="join-btn" :disabled="!!form.busy" @click="j.join">{{ form.busy === "join" ? HOME_JOIN_BUSY : buttonText }}</button>
    </div>
    </div>
    <a href="#" class="link-more" id="create-toggle" v-if="hasNet" @click.prevent="j.toggleCreate">{{ HOME_CREATE_TOGGLE }}</a>
    <div id="create-box" :hidden="!showCreate">
      <label class="field-label" for="create-label-input">{{ HOME_CREATE_LABEL }}</label>
      <input type="text" class="input" :class="{ 'field-error': form.labelError }" id="create-label-input" v-model="form.label" :placeholder="HOME_CREATE_PLACEHOLDER">
      <p class="field-error-text" id="create-label-error" role="alert" :aria-label="HOME_CREATE_ERROR_ARIA" :hidden="!form.labelError">{{ form.labelError }}</p>
      <div class="btn-row">
        <button type="button" :class="['btn', { 'btn-primary': !hasNet }]" id="create-btn" :disabled="!!form.busy" @click="j.create">{{ form.busy === "create" ? HOME_CREATE_BUSY : HOME_CREATE_BTN }}</button>
      </div>
    </div>
    <p class="hint" id="join-status" :hidden="!form.status">{{ form.status }}</p>
    <p class="hint" id="join-wait" role="status" :hidden="!waitText">{{ waitText }}</p>
  </div>
</template>
