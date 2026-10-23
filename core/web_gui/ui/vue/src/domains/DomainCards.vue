<script setup lang="ts">
// "Site checks" on a narrow screen: one card per site instead of the wide table.
import type { MergedDomain, SiteNet } from "./domains-view.ts";
import { DOMAINS_EMPTY } from "./domains-texts.ts";
import DomainCard from "./DomainCard.vue";

defineProps<{
  domains: MergedDomain[];
  nets: SiteNet[];
  proxy: string;
  chosen: string[];
  busy: Record<string, boolean>;
  errors: Record<string, string>;
}>();
const emit = defineEmits<{
  (e: "recheck", name: string): void;
  (e: "remove", name: string): void;
  (e: "exit-via", idFull: string): void;
}>();
</script>

<template>
  <p v-if="domains.length === 0" class="hint">{{ DOMAINS_EMPTY }}</p>
  <div v-else class="domain-cards">
    <DomainCard v-for="d in domains" :key="d.name" :domain="d" :nets="nets" :proxy="proxy" :chosen="chosen" :busy="!!busy[d.name]" :error="errors[d.name] || ''" @recheck="emit('recheck', $event)" @remove="emit('remove', $event)" @exit-via="emit('exit-via', $event)" />
  </div>
</template>
