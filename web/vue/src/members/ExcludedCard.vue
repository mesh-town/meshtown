<script setup lang="ts">
// The screen of a computer that is no longer in the network: what happened and the way back (a new invite).
// Only shows the text and reports the click; switching to Home and focusing the invite field is the parent's job.
// In the Android app the way back is the app's "Start over" dialog: a link to an address the app intercepts.
import { inAppPage } from "../home/browser-setup.ts";
import {
  EXCLUDED_APP_BUTTON, EXCLUDED_APP_TEXT, EXCLUDED_JOIN, EXCLUDED_LEFT_TEXT, EXCLUDED_REMOVED_TEXT, START_OVER_URL, excludedTitle,
} from "./members-texts.ts";

const props = defineProps<{ variant: "removed" | "left"; net: string }>();
defineEmits<{ (e: "join"): void }>();
const inApp = inAppPage();
const body = (): string => (props.variant === "left" ? EXCLUDED_LEFT_TEXT : EXCLUDED_REMOVED_TEXT);
</script>

<template>
  <section id="excluded-card" class="card excluded-card" role="status">
    <h2>{{ excludedTitle(variant, net) }}</h2>
    <template v-if="inApp">
      <p>{{ EXCLUDED_APP_TEXT }}</p>
      <a id="excluded-start-over" class="btn btn-primary" role="button" :href="START_OVER_URL">{{ EXCLUDED_APP_BUTTON }}</a>
    </template>
    <template v-else>
      <p>{{ body() }}</p>
      <button id="excluded-join" type="button" class="btn btn-primary" @click="$emit('join')">{{ EXCLUDED_JOIN }}</button>
    </template>
  </section>
</template>
