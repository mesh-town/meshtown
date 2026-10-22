<script setup lang="ts">
// The card for a computer that asked to join and waits for confirmations (`me_pending`): the title and how many
// members confirmed it. Only draws.
import { PENDING_TITLE, meWaitingText } from "./members-texts.ts";

defineProps<{ votes: number; need: number }>();
</script>

<template>
  <section id="members-pending-card" class="card warn-card" role="region" aria-labelledby="members-pending-card-title">
    <h2 id="members-pending-card-title">{{ PENDING_TITLE }}</h2>
    <p id="members-pending-card-text">{{ meWaitingText(votes, need) }}</p>
  </section>
</template>
