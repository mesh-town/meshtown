<script setup lang="ts">
// The two small pictures of the invite window and of the pending-invites list: "copy" (text) and "qr" (QR code).
defineProps<{ kind: "copy" | "qr" }>();
</script>

<template>
  <svg class="site-icon" width="16" height="16" viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <template v-if="kind === 'copy'">
      <rect x="6" y="6" width="9" height="9" rx="1.5" /><path d="M12 3.5V3a1 1 0 0 0-1-1H3a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h.5" />
    </template>
    <template v-else>
      <rect x="2" y="2" width="5" height="5" /><rect x="11" y="2" width="5" height="5" /><rect x="2" y="11" width="5" height="5" /><path d="M11 11h2v2h-2zM15 15h1M11 15h1" />
    </template>
  </svg>
</template>
