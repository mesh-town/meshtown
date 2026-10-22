<script setup lang="ts">
// Caption after the networks are saved: "Saved…", the router hint, with `showRestart` — the restart button, then the plate.
import { BTN_RESTART, PLATE_BUSY, PLATE_DONE } from "./nets-labels.ts";
import { ROUTER_RESTART_NOTE, SAVED_NOTE } from "./nets-texts.ts";
import type { RestartPhase } from "./use-nets.ts";

defineProps<{ phase: RestartPhase; nodeIsRouter: boolean; showRestart: boolean }>();
defineEmits<{ (e: "restart"): void }>();
</script>

<template>
  <div id="nets-saved" class="nets-saved">
    <div v-if="phase === 'busy'" id="nets-plate" class="nets-plate">{{ PLATE_BUSY }}</div>
    <div v-else-if="phase === 'done'" id="nets-plate" class="nets-plate ok">{{ PLATE_DONE }}</div>
    <template v-else>
      <p id="nets-saved-note">{{ SAVED_NOTE }}</p>
      <p v-if="nodeIsRouter" id="nets-router-hint" class="hint">{{ ROUTER_RESTART_NOTE }}</p>
      <button v-if="showRestart" type="button" class="btn" id="nets-restart-btn" @click="$emit('restart')">{{ BTN_RESTART }}</button>
    </template>
  </div>
</template>
