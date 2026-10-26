<script setup lang="ts">
// The invite form under the card of a network this computer is out of: it joins in this tab's own slot, or leads to the slot
// the node picked. The title and the hint say which network the invite has to be for and who to ask.
import { computed, inject } from "vue";
import { STORE_KEY } from "../core/store.ts";
import InviteForm from "../home/InviteForm.vue";
import { HOME_ASK_ADMIN_HINT } from "../home/home-card-texts.ts";
import { quotedNet } from "../common/words.ts";
import { REFUSED_LEAD, REJOIN_TITLE } from "../home/invite-join-texts.ts";
import { useArrival, useInviteJoin } from "../home/use-invite-join.ts";
import type { InviteFormIds } from "../home/use-invite-join.ts";
import type { InviteOutcome } from "../home/invite-join-actions.ts";

const props = defineProps<{ slot: string; net: string; variant: "banned" | "left"; refused?: string | null }>();
const emit = defineEmits<{ attempt: [] }>();
const store = inject(STORE_KEY)!;
const invite = useInviteJoin(store, { kind: "rejoin", slot: props.slot });
const arrival = useArrival(store);
const baseIds: InviteFormIds = {
  input: "rejoin-input", button: "rejoin-btn", error: "rejoin-error", preview: "rejoin-kind-preview", note: "rejoin-place-note",
  dup: "rejoin-dup", open: "rejoin-open", status: "rejoin-status", title: "rejoin-title",
};

// The waiting line is stale once the join is refused: the refusal under the button replaces it.
const ids = computed<InviteFormIds>(() => (props.refused ? { ...baseIds, status: "" } : baseIds));

const title = computed(() => REJOIN_TITLE(props.net));
const hint = computed(() => HOME_ASK_ADMIN_HINT(props.net));
const arrived = (outcome: InviteOutcome, slot: string | null): void => {
  if (slot && slot !== props.slot) arrival(outcome, invite.form);
};

defineExpose({ focus: invite.focus });
</script>

<template>
  <div id="rejoin-card" class="card" role="region" aria-labelledby="rejoin-title">
    <InviteForm :join="invite" :ids="ids" :title="title" @done="arrived" @start="emit('attempt')">
      <template #hint>
        <p class="hint" id="rejoin-hint">{{ hint }}</p>
      </template>
      <template #after>
        <p v-if="refused" id="rejoin-refused" class="field-error-text" role="alert"><strong>{{ REFUSED_LEAD(quotedNet(net)) }}</strong> {{ refused }}</p>
      </template>
    </InviteForm>
  </div>
</template>
