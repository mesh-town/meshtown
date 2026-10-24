<script setup lang="ts">
// The one "Join with an invite" form: the field, its error, what the string leads to, the button, the answer. The places
// (Home, "Add another network", the tab of a network the computer left) give the ids, the title and the hint under the note.
import { computed, inject } from "vue";
import { STORE_KEY } from "../core/store.ts";
import { peersHash } from "../shell/nav.ts";
import { duplicateText, openNetText } from "./add-net-texts.ts";
import { HOME_JOIN_BUSY, HOME_JOIN_ERROR_ARIA, HOME_JOIN_FIELD_LABEL, HOME_JOIN_PASTE_HINT, HOME_JOIN_PLACEHOLDER } from "./home-card-texts.ts";
import { HOME_BUTTON_JOIN } from "./home-flow-texts.ts";
import { netTitle } from "./invite-join-actions.ts";
import type { InviteOutcome } from "./invite-join-actions.ts";
import { highlightParts } from "./join-actions.ts";
import type { InviteFormIds, InviteJoin } from "./use-invite-join.ts";

const props = defineProps<{ join: InviteJoin; ids: InviteFormIds; title?: string; button?: string; busy?: string }>();
const emit = defineEmits<{ done: [outcome: InviteOutcome, slot: string | null] }>();
const store = inject(STORE_KEY)!;
const { form, input, preview, note, noteLink, outcome } = props.join;
const errorParts = computed(() => (form.inviteError ? highlightParts(form.inviteError.message, form.inviteError.word) : []));
const dupName = computed(() => netTitle(store.nets, form.dupSlot, form.netLabel));
const describedBy = computed(() => `${props.ids.error} ${props.ids.note}`);

const setInput = (el: unknown): void => {
  input.value = el as HTMLInputElement | null;
};
const open = (slot: string | undefined): void => {
  if (slot) location.hash = peersHash(slot);
};

async function go(): Promise<void> {
  const result = await props.join.submit();
  emit("done", result, form.slot ?? null);
}
</script>

<template>
  <div class="invite-form">
    <h3 v-if="title && ids.title" :id="ids.title" class="invite-form-title">{{ title }}</h3>
    <label class="field-label" :for="ids.input">{{ HOME_JOIN_FIELD_LABEL }}</label>
    <input :ref="setInput" type="text" class="input mono input-wide" :class="{ 'field-error': form.inviteError }" :id="ids.input" v-model="form.inviteInput" autocomplete="off" spellcheck="false" :aria-describedby="describedBy" :placeholder="HOME_JOIN_PLACEHOLDER">
    <p class="field-error-text" :id="ids.error" role="alert" :aria-label="HOME_JOIN_ERROR_ARIA" :hidden="!form.inviteError"><template v-for="(part, i) in errorParts" :key="i"><mark v-if="part.highlighted" class="key-error-word">{{ part.text }}</mark><template v-else>{{ part.text }}</template></template></p>
    <p v-if="preview" class="hint kind-preview" :id="ids.preview" role="status">{{ preview }}</p>
    <p v-if="note" class="hint place-note" :id="ids.note">{{ note }}<template v-if="noteLink"> <a href="#" class="link-more" @click.prevent="open(noteLink.slot)">{{ noteLink.text }}</a></template></p>
    <slot name="hint"></slot>
    <p class="hint">{{ HOME_JOIN_PASTE_HINT }}</p>
    <div class="btn-row">
      <button type="button" class="btn btn-primary" :id="ids.button" :disabled="!!form.busy" @click="go">{{ form.busy ? busy ?? HOME_JOIN_BUSY : button ?? HOME_BUTTON_JOIN }}</button>
    </div>
    <p v-if="ids.status" class="hint" :id="ids.status" :hidden="!form.status">{{ form.status }}</p>
    <div v-if="outcome === 'here' && form.dupSlot" :id="ids.dup" role="status">
      <p class="hint">{{ duplicateText(dupName) }}</p>
      <button type="button" class="btn" :id="ids.open" @click="open(form.dupSlot)">{{ openNetText(dupName) }}</button>
    </div>
  </div>
</template>
