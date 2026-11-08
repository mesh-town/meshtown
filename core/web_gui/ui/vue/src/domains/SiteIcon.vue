<script setup lang="ts">
// The glyph of a table button: an inline SVG that takes the text color (recheck arrow, remove cross).
defineProps<{ kind: "recheck" | "remove" }>();
</script>

<template>
  <svg class="site-icon" width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <template v-if="kind === 'recheck'">
      <path d="M15 9a6 6 0 1 1-1.8-4.3" />
      <path d="M15 2.5v3.2h-3.2" />
    </template>
    <path v-else d="M4.5 4.5l9 9M13.5 4.5l-9 9" />
  </svg>
</template>
