<script setup lang="ts">
// The line above the member list of a Managed network: `Managed network · Admins: you, anna`.
import { computed } from "vue";
import { adminsLineText } from "./admins-line.ts";

const props = defineProps<{ admins: string[]; me: string; names: Record<string, string> }>();
const text = computed(() => adminsLineText(props.admins, props.me, props.names));
</script>

<template>
  <p class="hint admins-line" id="admins-line">{{ text }}</p>
</template>
