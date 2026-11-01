<script setup lang="ts">
// The screen of a computer that is no longer in the network: what happened and the way back (a new invite).
// Only shows the text and reports the click; switching to Home and focusing the invite field is the parent's job.
import { excludedByText } from "./excluded-by.ts";
import { forDevice } from "../common/device.ts";
import {
  EXCLUDED_JOIN, EXCLUDED_LEFT_TEXT, EXCLUDED_NO_REASON, EXCLUDED_REASON_LABEL, EXCLUDED_REMOVED_TEXT, excludedTitle,
} from "./members-texts.ts";

// `members` — the card on the Members page: its ids differ from the Home card, both pages live in one document.
const props = defineProps<{ variant: "banned" | "left"; net: string; by?: string | null; members?: boolean; formOpen?: boolean; reason?: string | null }>();
defineEmits<{ (e: "join"): void }>();
// An admin removed the computer, or it left on its own.
const rawBody = (): string => (props.by ? excludedByText(props.by, props.net) : props.variant === "banned" ? EXCLUDED_REMOVED_TEXT : EXCLUDED_LEFT_TEXT);
const body = (): string => forDevice(rawBody());
// Only for a computer an admin removed; `undefined` — the node does not say (older version), so no line. Text only, never markup.
const showReason = (): boolean => props.variant === "banned" && props.reason !== undefined;
const id = (name: string): string => (props.members ? "members-removed-" : "excluded-") + name;
</script>

<template>
  <section :id="id('card')" class="card excluded-card" role="status">
    <h2>{{ excludedTitle(variant, net) }}</h2>
    <p>{{ body() }}</p>
    <p v-if="showReason()" id="excluded-reason">{{ reason === null ? EXCLUDED_NO_REASON : EXCLUDED_REASON_LABEL + reason }}</p>
    <button v-if="!formOpen" :id="id('join')" type="button" class="btn btn-primary" @click="$emit('join')">{{ EXCLUDED_JOIN }}</button>
  </section>
</template>
