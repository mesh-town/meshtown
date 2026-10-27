<script setup lang="ts">
// Text where the words PAC and SOCKS5 carry a short explanation on hover or focus; Escape dismisses it.
import { useId } from "vue";
import { termParts } from "./term-hints.ts";

defineProps<{ text: string }>();
const uid = useId();

function dismiss(e: KeyboardEvent): void {
  (e.currentTarget as HTMLElement).blur();
}
</script>

<template>
  <template v-for="(part, i) in termParts(text)" :key="i">
    <span v-if="part.tip" class="term" tabindex="0" :aria-describedby="`${uid}-${i}`" @keydown.esc="dismiss">{{ part.text }}<span class="term-tip" role="tooltip" :id="`${uid}-${i}`">{{ part.tip }}</span></span>
    <template v-else>{{ part.text }}</template>
  </template>
</template>
