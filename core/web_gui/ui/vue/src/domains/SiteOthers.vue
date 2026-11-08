<script setup lang="ts">
// The "Other exits" line: plain text plus, when there is a counter ("3 more"), a button that opens the same list as "Details".
import type { OthersText } from "./domains-texts.ts";

defineProps<{ others: OthersText; open: boolean }>();
defineEmits<{ (e: "toggle"): void }>();
</script>

<template>
  <span class="hint site-others">{{ others.more ? others.lead + " " : others.lead }}<button v-if="others.more" type="button" class="site-more" :aria-expanded="open" @click="$emit('toggle')">{{ others.more }}</button></span>
</template>
