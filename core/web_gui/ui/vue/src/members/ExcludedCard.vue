<script setup lang="ts">
// The screen of a computer that is no longer in the network: what happened and the way back (a new invite).
// Only shows the text and reports the click; switching to Home and focusing the invite field is the parent's job.
// In the Android app the way back is the app's "Start over" dialog: a link to an address the app intercepts.
import { excludedByText } from "./excluded-by.ts";
import { inAppPage } from "../home/browser-setup.ts";
import {
  EXCLUDED_APP_BUTTON, EXCLUDED_APP_TEXT, EXCLUDED_JOIN, EXCLUDED_LEFT_TEXT, EXCLUDED_BANNED_TEXT, EXCLUDED_REMOVED_TEXT, FORGOTTEN_CARD_TEXT, FORGOTTEN_JOIN, START_OVER_URL, excludedTitle,
} from "./members-texts.ts";

// `members` — the card on the Members page: its ids differ from the Home card, both pages live in one document.
const props = defineProps<{ variant: "banned" | "left" | "forgotten"; net: string; by?: string | null; members?: boolean; managed?: boolean }>();
defineEmits<{ (e: "join"): void }>();
const inApp = inAppPage();
// In a Managed network an admin removes a member; in a Peer network the members ban one.
const BODY = { left: EXCLUDED_LEFT_TEXT, banned: EXCLUDED_BANNED_TEXT, forgotten: FORGOTTEN_CARD_TEXT };
const bannedText = (): string => (props.managed ? EXCLUDED_REMOVED_TEXT : EXCLUDED_BANNED_TEXT);
const body = (): string => (props.by ? excludedByText(props.by, props.net) : props.variant === "banned" ? bannedText() : BODY[props.variant]);
// Removed from the member list: a plain invite is the way back, also in the app (no "Start over" there).
const id = (name: string): string => (props.members ? "members-removed-" : "excluded-") + name;
const startOver = (): boolean => inApp && props.variant !== "forgotten";
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
      <button :id="id('join')" type="button" class="btn btn-primary" @click="$emit('join')">{{ props.variant === "forgotten" ? FORGOTTEN_JOIN : EXCLUDED_JOIN }}</button>
    </template>
  </section>
</template>
