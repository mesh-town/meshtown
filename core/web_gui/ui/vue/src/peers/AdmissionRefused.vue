<script setup lang="ts">
// The plate over the member list: the members this computer knows turned it down and nobody else is online.
// It shows only when that has held for a while: right after joining, "0 online" is normal.
import { computed, onUnmounted, ref, watch } from "vue";
import type { MembersReply } from "../members/members-types.ts";
import { admissionRefused } from "./admission-refused.ts";
import {
  ADMISSION_REFUSED_DELAY_MS, ADMISSION_REFUSED_MANY, ADMISSION_REFUSED_ONE, ADMISSION_REFUSED_TITLE, admissionRefusedAsk, admissionRefusedList,
} from "./peers-texts.ts";

const props = defineProps<{ members: MembersReply | null | undefined; net: string }>();
const refused = computed(() => admissionRefused(props.members));
const standing = computed(() => refused.value !== null);
const shown = ref(false);
let timer: ReturnType<typeof setTimeout> | null = null;

function stop(): void {
  if (timer) clearTimeout(timer);
  timer = null;
}

watch(standing, (now) => {
  stop();
  shown.value = false;
  if (now) timer = setTimeout(() => (shown.value = true), ADMISSION_REFUSED_DELAY_MS);
}, { immediate: true });
onUnmounted(stop);

const names = computed(() => refused.value?.names ?? []);
const what = computed(() => (names.value.length === 1 ? ADMISSION_REFUSED_ONE(names.value[0], props.net) : ADMISSION_REFUSED_MANY(admissionRefusedList(names.value), props.net)));
const ask = computed(() => admissionRefusedAsk(names.value, props.net, props.members?.kind === "Managed"));
</script>

<template>
  <section v-if="shown && refused" id="admission-refused" class="card warn-card" role="status">
    <h2>{{ ADMISSION_REFUSED_TITLE(net) }}</h2>
    <p>{{ what }}</p>
    <p>{{ ask }}</p>
  </section>
</template>
