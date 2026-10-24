<script setup lang="ts">
// The collapsed group under the member list: nodes that are no longer in the network, one ready line each.
// Renders nothing without lines; the open / closed state belongs to the browser and survives a re-render.
import { excludedSummary } from "./members-texts.ts";

defineProps<{ lines: string[] }>();
</script>

<template>
  <details v-if="lines.length > 0" id="excluded-list" class="card removed">
    <summary>{{ excludedSummary(lines.length) }}</summary>
    <ul>
      <li v-for="(line, i) in lines" :key="i">{{ line }}</li>
    </ul>
  </details>
</template>
