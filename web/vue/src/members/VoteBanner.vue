<script setup lang="ts">
// The banner on top of the Members tab: somebody proposed removing a member and this member can confirm.
// Only shows the text and reports the clicks; the dialog and "hide until the page is reloaded" are the parent's job.
import { VOTE_CONFIRM, VOTE_LATER, reasonLine, voteBody, voteTitle } from "./members-texts.ts";

defineProps<{ by: string; target: string; reason?: string | null }>();
defineEmits<{ (e: "confirm"): void; (e: "later"): void }>();
</script>

<template>
  <section id="vote-banner" class="card warn-card" role="region" aria-labelledby="vote-title" tabindex="-1">
    <h2 id="vote-title">{{ voteTitle(by, target) }}</h2>
    <p>{{ voteBody(target) }}</p>
    <p v-if="reason" class="vote-reason">{{ reasonLine(by, reason) }}</p>
    <div class="btn-row">
      <button id="vote-confirm" type="button" class="btn btn-primary" @click="$emit('confirm')">{{ VOTE_CONFIRM }}</button>
      <button id="vote-later" type="button" class="btn" @click="$emit('later')">{{ VOTE_LATER }}</button>
    </div>
  </section>
</template>
