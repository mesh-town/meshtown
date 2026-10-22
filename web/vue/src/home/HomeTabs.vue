<script setup lang="ts">
// The two tabs of "Home": "Main" and "Setup". The ←/→ arrows move focus to the neighboring tab
// and open it; the choice is set by the shared `useHome` (`homeTab`).
import { inject } from "vue";
import { STORE_KEY } from "../core/store.ts";
import { HOME_TAB_TEXTS, HOME_TABS_LABEL } from "./home-words.ts";
import type { HomeTab } from "./home-view.ts";
import { useHomeContext } from "./use-home.ts";

const { activeTab, homeTab } = useHomeContext(inject(STORE_KEY)!);

function pick(tab: HomeTab): void {
  homeTab.value = tab;
}

// The arrow moves cyclically; the neighbor's `id` is taken from the order in `HOME_TAB_TEXTS`.
function arrow(event: KeyboardEvent, index: number): void {
  const step = event.key === "ArrowRight" ? 1 : event.key === "ArrowLeft" ? -1 : 0;
  if (!step) return;
  event.preventDefault();
  const [id] = HOME_TAB_TEXTS[(index + step + HOME_TAB_TEXTS.length) % HOME_TAB_TEXTS.length];
  pick(id);
  document.getElementById("home-tab-" + id)?.focus();
}
</script>

<template>
  <div class="home-tabs" role="tablist" :aria-label="HOME_TABS_LABEL">
    <button
      v-for="([id, text], i) in HOME_TAB_TEXTS"
      :id="'home-tab-' + id"
      :key="id"
      type="button"
      class="home-tab"
      :class="{ active: activeTab === id }"
      role="tab"
      aria-controls="home-tabpanel"
      :aria-selected="activeTab === id ? 'true' : 'false'"
      :tabindex="activeTab === id ? 0 : -1"
      @click="pick(id)"
      @keydown="arrow($event, i)"
    >{{ text }}</button>
  </div>
</template>
