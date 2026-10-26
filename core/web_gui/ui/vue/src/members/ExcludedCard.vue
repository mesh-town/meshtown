<script setup lang="ts">
// The screen of a computer that is no longer in the network: what happened and the way back (a new invite).
// Only shows the text and reports the click; switching to Home and focusing the invite field is the parent's job.
// In the Android app the way back is the app's "Start over" dialog: a link to an address the app intercepts.
import { excludedByText } from "./excluded-by.ts";
import { inAppPage } from "../home/browser-setup.ts";
import { forDevice } from "../common/device.ts";
import {
  EXCLUDED_APP_BUTTON, EXCLUDED_APP_TEXT, EXCLUDED_JOIN, EXCLUDED_LEFT_TEXT, EXCLUDED_REMOVED_TEXT, START_OVER_URL, excludedTitle,
} from "./members-texts.ts";

// `members` — the card on the Members page: its ids differ from the Home card, both pages live in one document.
const props = defineProps<{ variant: "banned" | "left"; net: string; by?: string | null; members?: boolean; formOpen?: boolean }>();
defineEmits<{ (e: "join"): void }>();
const inApp = inAppPage();
// An admin removed the computer, or it left on its own.
const rawBody = (): string => (props.by ? excludedByText(props.by, props.net) : props.variant === "banned" ? EXCLUDED_REMOVED_TEXT : EXCLUDED_LEFT_TEXT);
const body = (): string => forDevice(rawBody());
const id = (name: string): string => (props.members ? "members-removed-" : "excluded-") + name;
const startOver = (): boolean => inApp;
</script>

<template>
  <section :id="id('card')" class="card excluded-card" role="status">
    <h2>{{ excludedTitle(variant, net) }}</h2>
    <template v-if="startOver()">
      <p>{{ EXCLUDED_APP_TEXT }}</p>
      <a :id="id('start-over')" class="btn btn-primary" role="button" :href="START_OVER_URL">{{ EXCLUDED_APP_BUTTON }}</a>
    </template>
    <template v-else>
      <p>{{ body() }}</p>
      <button v-if="!formOpen" :id="id('join')" type="button" class="btn btn-primary" @click="$emit('join')">{{ EXCLUDED_JOIN }}</button>
    </template>
  </section>
</template>
