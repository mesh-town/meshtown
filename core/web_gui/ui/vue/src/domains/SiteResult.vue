<script setup lang="ts">
// The result of one site for your browser: the mark and its age, or why there is none.
// Shared by the wide table row and the narrow card.
import {
  DOMAINS_CHECKING_ROW, DOMAINS_EXIT_NOT_SHARING, DOMAINS_EXIT_OFFLINE, DOMAINS_NOT_CHECKED,
} from "./domains-texts.ts";
import type { SiteSummary } from "./domains-summary.ts";

defineProps<{ sum: SiteSummary }>();
</script>

<template>
  <span v-if="sum.kind === 'checking'" class="hint">{{ DOMAINS_CHECKING_ROW }}</span>
  <span v-else-if="sum.kind === 'exit-offline'" class="badge badge-down">{{ DOMAINS_EXIT_OFFLINE }}</span>
  <span v-else-if="sum.kind === 'exit-not-sharing'" class="badge badge-down">{{ DOMAINS_EXIT_NOT_SHARING }}</span>
  <template v-else-if="sum.mark">
    <span class="badge" :class="sum.mark.cls" :title="sum.mark.title">{{ sum.mark.text }}</span>
    <span v-if="sum.kind === 'unknown'" class="hint">{{ " " + DOMAINS_NOT_CHECKED }}</span>
    <span v-else-if="sum.age" class="hint"> · {{ sum.age }}</span>
  </template>
</template>
