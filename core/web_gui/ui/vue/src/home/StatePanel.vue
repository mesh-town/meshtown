<script setup lang="ts">
// The "how sites open right now" panel: the text and buttons come from `panel` (`statePanel`).
import { HOME_PANEL_ARIA, HOME_PANEL_DIRECT, HOME_PANEL_PICK_OTHER } from "./home-card-texts.ts";
import type { StatePanelView } from "./home-view.ts";
import RingMark from "./RingMark.vue";
import { useRingSize } from "./ring-view.ts";
import type { RingView } from "./ring-view.ts";

defineProps<{ panel: StatePanelView | null; ring?: RingView | null }>();
const dims = useRingSize();
defineEmits<{ (e: "pick-other"): void; (e: "direct"): void }>();
</script>

<template>
  <div v-if="panel" class="card state-panel" :class="ring ? ['with-ring', 'ring-level-' + panel.level] : null" role="region" :aria-label="HOME_PANEL_ARIA">
    <span v-if="panel.busy" class="spinner state-spinner" aria-hidden="true"></span>
    <RingMark v-else-if="ring" :members="ring.members" :size="dims.size" :thickness="dims.thickness" :center="ring.center" />
    <span v-else class="state-dot" :class="panel.level" aria-hidden="true"></span>
    <div>
      <div class="state-main" id="route-state">{{ panel.main }}</div>
      <div v-if="panel.sub" class="state-sub">{{ panel.sub }}</div>
      <ul v-if="ring && ring.legend.length" class="ring-legend">
        <li v-for="item in ring.legend" :key="item.role" :class="'ring-legend-' + item.role">{{ item.text }}</li>
      </ul>
      <div v-if="panel.actions.length" class="btn-row">
        <button v-if="panel.actions.includes('pick-other')" type="button" class="btn" @click="$emit('pick-other')">{{ HOME_PANEL_PICK_OTHER }}</button>
        <button v-if="panel.actions.includes('direct')" type="button" class="btn" @click="$emit('direct')">{{ HOME_PANEL_DIRECT }}</button>
      </div>
    </div>
  </div>
</template>
