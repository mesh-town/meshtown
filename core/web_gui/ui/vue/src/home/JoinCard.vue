<script setup lang="ts">
// The join card: one component for two places — "Home" without a network (the form is chosen by the setup's panel
// `#home-start-choice`) and "Settings" with a network (`joinPlace`). `join` is a ready `useJoin` from the parent (the setup drives the form itself), otherwise its own.
import { computed, inject } from "vue";
import { STORE_KEY } from "../core/store.ts";
import NetKindPicker from "./NetKindPicker.vue";
import InviteForm from "./InviteForm.vue";
import { isInvite } from "./invite-join-actions.ts";
import { useArrival } from "./use-invite-join.ts";
import type { InviteOutcome } from "./invite-join-actions.ts";
import type { InviteFormIds } from "./use-invite-join.ts";
import { netName } from "../core/slots.ts";
import { isMeExcluded } from "../members/members-ids.ts";
import { useJoin } from "./use-join.ts";
import { useRefusalGate } from "./use-refusal-gate.ts";
import { inviteWaitText, refusedLead, refusedText } from "./join-wait.ts";
import type { JoinView } from "./use-join.ts";
import {
  HOME_ASK_ADMIN_HINT, HOME_ASK_MEMBER_HINT, HOME_CREATE_BTN, HOME_CREATE_BUSY, HOME_CREATE_ERROR_ARIA, HOME_CREATE_LABEL, HOME_CREATE_PLACEHOLDER,
  HOME_CREATE_TOGGLE, HOME_JOIN_KEY_HINT, HOME_NO_RETURN_HINT,
} from "./home-card-texts.ts";
import { HOME_BUTTON_JOIN } from "./home-flow-texts.ts";
import { REJOIN_OTHER, REJOIN_TITLE } from "./invite-join-texts.ts";

const props = defineProps<{ join?: JoinView | null; quiet?: boolean; removed?: boolean }>();
const emit = defineEmits<{ attempt: [] }>();
const store = inject(STORE_KEY)!;
const j = props.join || useJoin(store);
const { form, hasNet, showJoin, showCreate, title, buttonText } = j;
// `quiet` — the setup has already shown the join progress in its own block, so we don't duplicate the line under the form.
// `removed` — the others removed this computer from the member list; an excluded computer is told by the member list too:
// either way the title and the hint say where the invite has to come from.
const ids: InviteFormIds = {
  input: "join-key-input", button: "join-btn", error: "join-key-error", preview: "join-kind-preview", note: "join-place-note", dup: "join-dup", open: "join-open",
};
const arrival = useArrival(store);
const arrived = (outcome: InviteOutcome, slot: string | null): void => {
  if (slot && slot !== "main") arrival(outcome, form);
};
const gone = computed(() => !!props.removed || (hasNet.value && isMeExcluded(store.state, store.members)));
const managed = computed(() => store.state?.network?.kind === "Managed");
const bannedPeer = computed(() => gone.value && !props.removed && !managed.value);
const netLabel = computed(() => netName(store.nets, store.lead));
const heading = computed(() => (!gone.value ? title.value : bannedPeer.value ? REJOIN_OTHER : REJOIN_TITLE(netLabel.value)));
const button = computed(() => (gone.value ? HOME_BUTTON_JOIN : buttonText.value));
// Under the field: where a new invite has to come from, or — for a word key — that it replaces the network.
const hint = computed(() => {
  if (bannedPeer.value) return HOME_NO_RETURN_HINT(netLabel.value);
  if (gone.value) return (managed.value ? HOME_ASK_ADMIN_HINT : HOME_ASK_MEMBER_HINT)(netLabel.value);
  return hasNet.value && form.inviteInput.trim() && !isInvite(form.inviteInput) ? HOME_JOIN_KEY_HINT(netLabel.value) : "";
});
const refusal = useRefusalGate(() => store.state?.node?.invite_join);
const attempt = (): void => {
  refusal.dismiss();
  emit("attempt");
};
const leadText = computed(() => (props.quiet ? "" : refusedLead(refusal.info.value)));
// A refusal is told as an error right under the Join button; the waiting line stays below the card.
const refusedNow = computed(() => !props.quiet && refusedText(refusal.info.value) !== null);
const waitText = computed(() => (props.quiet ? "" : inviteWaitText(refusal.info.value)));
// "Invite accepted. Waiting…" is stale once the node reports a refusal, and doubles the waiting line: one line stays.
const statusText = computed(() => (refusedText(refusal.info.value) === null && !waitText.value ? form.status : ""));
</script>

<template>
  <div id="join-card" role="region" aria-labelledby="join-card-title">
    <div class="step-head"><h2 id="join-card-title">{{ heading }}</h2></div>
    <div id="join-box" :hidden="!showJoin">
      <InviteForm :join="j.invite" :ids="ids" :button="button" @done="arrived" @start="attempt">
        <template #hint>
          <p class="hint" id="net-restart-hint" :hidden="!hint">{{ hint }}</p>
        </template>
        <template #after>
          <p v-if="refusedNow" class="field-error-text" id="join-wait" role="alert"><strong>{{ leadText }}</strong> {{ waitText }}</p>
        </template>
      </InviteForm>
    </div>
    <a href="#" class="link-more" id="create-toggle" v-if="hasNet" @click.prevent="j.toggleCreate">{{ HOME_CREATE_TOGGLE }}</a>
    <div id="create-box" :hidden="!showCreate">
      <label class="field-label" for="create-label-input">{{ HOME_CREATE_LABEL }}</label>
      <input type="text" class="input" :class="{ 'field-error': form.labelError }" id="create-label-input" v-model="form.label" :placeholder="HOME_CREATE_PLACEHOLDER">
      <p class="field-error-text" id="create-label-error" role="alert" :aria-label="HOME_CREATE_ERROR_ARIA" :hidden="!form.labelError">{{ form.labelError }}</p>
      <NetKindPicker v-model="form.kind" :name="form.label" />
      <div class="btn-row">
        <button type="button" :class="['btn', { 'btn-primary': !hasNet }]" id="create-btn" :disabled="!!form.busy" @click="j.create">{{ form.busy === "create" ? HOME_CREATE_BUSY : HOME_CREATE_BTN }}</button>
      </div>
    </div>
    <p class="hint" id="join-status" :hidden="!statusText">{{ statusText }}</p>
    <p v-if="!refusedNow" class="hint" id="join-wait" role="status" :hidden="!waitText">{{ waitText }}</p>
  </div>
</template>
