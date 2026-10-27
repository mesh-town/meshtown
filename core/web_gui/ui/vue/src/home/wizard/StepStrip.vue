<script setup lang="ts">
// The strip of steps above the wizard screen: passed steps (✓) and steps seen before are buttons, the rest are plain.
import type { StepId, StripStep } from "./wizard-steps.ts";
import { WIZ_STEP_NAMES, WIZ_STRIP_LABEL } from "./wizard-texts.ts";

defineProps<{ steps: StripStep[]; hasNet: boolean }>();
defineEmits<{ go: [id: StepId] }>();
const clickable = (s: StripStep, hasNet: boolean): boolean =>
  s.state === "open" || (s.state === "done" && !(s.id === "join" && hasNet));
</script>

<template>
  <ol id="wizard-strip" :aria-label="WIZ_STRIP_LABEL">
    <li v-for="(s, i) in steps" :key="s.id" :class="['strip-step', s.state]">
      <button v-if="clickable(s, hasNet)" type="button" :id="`wizard-strip-${s.id}`" @click="$emit('go', s.id)">
        <span class="strip-dot" aria-hidden="true">{{ s.state === "done" ? "✓" : i + 1 }}</span>
        <span class="strip-name">{{ WIZ_STEP_NAMES[s.id] }}</span>
      </button>
      <span v-else :id="`wizard-strip-${s.id}`" :aria-current="s.state === 'current' ? 'step' : undefined">
        <span class="strip-dot" aria-hidden="true">{{ s.state === "done" ? "✓" : i + 1 }}</span>
        <span class="strip-name">{{ WIZ_STEP_NAMES[s.id] }}</span>
      </span>
    </li>
  </ol>
</template>
