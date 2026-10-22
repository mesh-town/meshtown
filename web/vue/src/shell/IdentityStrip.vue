<script setup lang="ts">
// The identity strip: `identity` — `null | { cls, text }`.
import type { IdentityStripView } from "./status-line.ts";

withDefaults(defineProps<{ identity?: IdentityStripView | null }>(), { identity: null });
</script>

<template>
  <div id="identity-strip" class="identity-strip" :class="identity && identity.cls" :hidden="!identity">{{ identity && identity.text }}</div>
</template>
