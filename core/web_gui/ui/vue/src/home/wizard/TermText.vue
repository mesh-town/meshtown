<script setup lang="ts">
// Text where the words PAC and SOCKS5 carry a short explanation on hover or focus; Escape dismisses it.
import { useId } from "vue";
import { termParts } from "./term-hints.ts";

defineProps<{ text: string }>();
const uid = useId();

// The hint opens under the word; near the right edge of the window it slides left so it stays inside.
function place(e: Event): void {
  const term = e.currentTarget as HTMLElement;
  const tip = term.querySelector<HTMLElement>(".term-tip");
  if (!tip) return;
  tip.style.setProperty("--tip-shift", "0px");
  tip.style.display = "block";
  const over = term.getBoundingClientRect().left + tip.offsetWidth - (document.documentElement.clientWidth - 12);
  tip.style.display = "";
  tip.style.setProperty("--tip-shift", `${-Math.max(0, over)}px`);
}

function dismiss(e: KeyboardEvent): void {
  (e.currentTarget as HTMLElement).blur();
}
</script>

<template>
  <template v-for="(part, i) in termParts(text)" :key="i">
    <span v-if="part.tip" class="term" tabindex="0" :aria-describedby="`${uid}-${i}`" @mouseenter="place" @focus="place" @keydown.esc="dismiss">{{ part.text }}<span class="term-tip" role="tooltip" :id="`${uid}-${i}`">{{ part.tip }}</span></span>
    <template v-else>{{ part.text }}</template>
  </template>
</template>
