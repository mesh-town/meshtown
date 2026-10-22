<script setup lang="ts">
// The member ring: SVG arcs by role, text in the center. Doesn't compute roles — receives ready members.
import { computed } from "vue";
import { ringArcs } from "./ring-arcs.ts";
import type { RingArc } from "./ring-arcs.ts";
import type { RingMember } from "./ring-members.ts";
import { arcTitle, ringLenBucket } from "./ring-text.ts";

const props = defineProps<{ members: RingMember[]; size: number; thickness: number; center?: { main: string; sub: string } | null }>();

const arcs = computed(() => ringArcs(props.members, props.size, props.thickness));
const hasVia = computed(() => props.members.some((m) => m.role === "via"));
const mid = computed(() => props.size / 2);
const radius = computed(() => (props.size - props.thickness) / 2);
// The class by the length of the count string: the font size shrinks so the text lies inside the circle (not :style — CSP).
const lenClass = computed(() => `ring-len-${ringLenBucket(props.center?.main ?? "")}`);

const point = (rad: number): string => `${mid.value + radius.value * Math.sin(rad)} ${mid.value - radius.value * Math.cos(rad)}`;

// A full circle isn't drawn as one SVG arc (start = end): two half-circles.
function pathOf(arc: RingArc): string {
  const r = radius.value;
  if (arc.endRad - arc.startRad >= Math.PI * 2 - 1e-9) return `M ${point(0)} A ${r} ${r} 0 1 1 ${point(Math.PI)} A ${r} ${r} 0 1 1 ${point(0)}`;
  const large = arc.endRad - arc.startRad > Math.PI ? 1 : 0;
  return `M ${point(arc.startRad)} A ${r} ${r} 0 ${large} 1 ${point(arc.endRad)}`;
}
</script>

<template>
  <div class="ring-mark" :class="lenClass">
    <svg :viewBox="`0 0 ${size} ${size}`" :width="size" :height="size" aria-hidden="true">
      <circle v-if="arcs.length === 0" class="ring-empty" :cx="mid" :cy="mid" :r="radius" fill="none" />
      <path v-for="(arc, i) in arcs" :key="i" :class="['ring-arc', 'ring-' + arc.role]" :d="pathOf(arc)" :stroke-width="thickness" fill="none">
        <title>{{ arcTitle(arc, hasVia) }}</title>
      </path>
    </svg>
    <template v-if="center">
      <span class="ring-center-main">{{ center.main }}</span>
      <span class="ring-center-sub">{{ center.sub }}</span>
    </template>
  </div>
</template>
