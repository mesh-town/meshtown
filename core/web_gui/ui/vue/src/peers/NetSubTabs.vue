<script setup lang="ts">
// The sub-tabs of a network tab: Members, Shared services, Device settings. The ←/→/Home/End keys move focus to
// a tab and open it; a click writes the address, the shell reads the choice back from it.
import { peersHash } from "../shell/nav.ts";
import { netSubViews } from "../shell/net-subs.ts";
import type { NetSub } from "../shell/net-subs.ts";
import { NET_SUBS_LABEL } from "./peers-texts.ts";

const props = defineProps<{ slot: string; active: NetSub; subs: NetSub[] }>();

const labels = new Map(netSubViews(false).map((v) => [v.id, v.label]));

function open(sub: NetSub): void {
  location.hash = peersHash(props.slot, sub);
}

function target(key: string, index: number): number {
  if (key === "Home") return 0;
  if (key === "End") return props.subs.length - 1;
  const step = key === "ArrowRight" ? 1 : key === "ArrowLeft" ? -1 : 0;
  return step ? (index + step + props.subs.length) % props.subs.length : -1;
}

function arrow(event: KeyboardEvent, index: number): void {
  const to = target(event.key, index);
  if (to < 0) return;
  event.preventDefault();
  const sub = props.subs[to];
  open(sub);
  document.getElementById("net-sub-" + sub)?.focus();
}
</script>

<template>
  <div class="home-tabs" id="net-subtabs" role="tablist" :aria-label="NET_SUBS_LABEL">
    <button
      v-for="(sub, i) in subs"
      :id="'net-sub-' + sub"
      :key="sub"
      type="button"
      class="home-tab"
      :class="{ active: active === sub }"
      role="tab"
      aria-controls="net-subpanel"
      :aria-selected="active === sub ? 'true' : 'false'"
      :tabindex="active === sub ? 0 : -1"
      @click="open(sub)"
      @keydown="arrow($event, i)"
    >{{ labels.get(sub) }}</button>
  </div>
</template>
