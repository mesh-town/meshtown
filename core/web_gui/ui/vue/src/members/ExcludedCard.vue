<script setup lang="ts">
// The screen of a computer that is no longer in the network: what happened and the way back (a new invite).
// Only shows the text and reports the click; switching to Home and focusing the invite field is the parent's job.
// In the Android app the way back is the app's "Start over" dialog: a link to an address the app intercepts.
import { inAppPage } from "../home/browser-setup.ts";
import {
  EXCLUDED_APP_BUTTON, EXCLUDED_APP_TEXT, EXCLUDED_JOIN, EXCLUDED_LEFT_TEXT, EXCLUDED_BANNED_TEXT, FORGOTTEN_CARD_TEXT, FORGOTTEN_JOIN, START_OVER_URL, excludedTitle,
} from "./members-texts.ts";

const props = defineProps<{ variant: "banned" | "left" | "forgotten"; net: string }>();
defineEmits<{ (e: "join"): void }>();
const inApp = inAppPage();
const BODY = { left: EXCLUDED_LEFT_TEXT, banned: EXCLUDED_BANNED_TEXT, forgotten: FORGOTTEN_CARD_TEXT };
const body = (): string => BODY[props.variant];
// Removed from the member list: a plain invite is the way back, also in the app (no "Start over" there).
const startOver = (): boolean => inApp && props.variant !== "forgotten";
</script>

<template>
  <section id="excluded-card" class="card excluded-card" role="status">
    <h2>{{ excludedTitle(variant, net) }}</h2>
    <template v-if="startOver()">
      <p>{{ EXCLUDED_APP_TEXT }}</p>
      <a id="excluded-start-over" class="btn btn-primary" role="button" :href="START_OVER_URL">{{ EXCLUDED_APP_BUTTON }}</a>
    </template>
    <template v-else>
      <p>{{ body() }}</p>
      <button id="excluded-join" type="button" class="btn btn-primary" @click="$emit('join')">{{ props.variant === "forgotten" ? FORGOTTEN_JOIN : EXCLUDED_JOIN }}</button>
    </template>
  </section>
</template>
