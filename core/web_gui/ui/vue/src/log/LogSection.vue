<script setup lang="ts">
import { computed, inject, ref, watch } from "vue";
import { navigateTo } from "../common/navigate.ts";
import { STORE_KEY } from "../core/store.ts";
import type { Store } from "../core/store.ts";
import { getToken } from "../core/token.ts";
import { GAP_TEXT, chipTags, downloadUrl, formatLogTime, ringHint, visibleEntries } from "./log-view.ts";
import {
  LOG_AUTOSCROLL, LOG_DOWNLOAD, LOG_LOADING, LOG_PAUSE, LOG_RESUME, LOG_SEARCH_PLACEHOLDER, LOG_STORE_MISSING, LOG_TAG_ALL, LOG_WINDOW_LABEL,
} from "./log-texts.ts";
import { useLogFeed } from "./use-log-feed.ts";

const AT_BOTTOM_PX = 24;

const injected = inject(STORE_KEY);
if (!injected) throw new Error(LOG_STORE_MISSING);
const store: Store = injected;
const { feed, paused, pollError, togglePause } = useLogFeed(store);
const tag = ref<string>("all");
const search = ref<string>("");
const autoscroll = ref(true);
const userScrolledUp = ref(false);
const logWindowEl = ref<HTMLElement | null>(null);

const tags = computed(() => chipTags(store.state && store.state.log));
const hint = computed(() => ringHint(store.state && store.state.log));
const shown = computed(() => visibleEntries(feed.value.lines, tag.value, search.value));

function scrollDown(): void {
  const el = logWindowEl.value;
  if (el && autoscroll.value && !userScrolledUp.value) el.scrollTop = el.scrollHeight;
}

function onScroll(): void {
  const el = logWindowEl.value;
  if (!el) return;
  userScrolledUp.value = el.scrollHeight - el.scrollTop - el.clientHeight >= AT_BOTTOM_PX;
}

function setAutoscroll(on: boolean): void {
  autoscroll.value = on;
  if (on) userScrolledUp.value = false;
}

function download(): void {
  navigateTo(downloadUrl(getToken()));
}

// The tag is not in the list of the node's new start — the filter must not stick, we return to "all".
watch(tags, (list) => {
  if (tag.value !== "all" && !list.includes(tag.value)) tag.value = "all";
});
watch(() => store.state && store.state.boot_id, (now, prev) => {
  if (prev != null && now !== prev) tag.value = "all";
});
watch([shown, autoscroll], scrollDown, { flush: "post" });
</script>

<template>
  <div v-if="store.state" class="card">
    <div class="log-toolbar">
      <div class="chips" id="log-chips">
        <button type="button" class="chip" :class="{ active: tag === 'all' }" data-tag="all" @click="tag = 'all'">{{ LOG_TAG_ALL }}</button>
        <button v-for="t in tags" :key="t" type="button" class="chip" :class="{ active: tag === t }" :data-tag="t" @click="tag = t">{{ t }}</button>
      </div>
    </div>
    <div class="log-toolbar">
      <input type="text" class="input log-search" id="log-search-input" :placeholder="LOG_SEARCH_PLACEHOLDER" @input="search = ($event.target as HTMLInputElement).value.trim()">
      <span class="toggle-inline">
        <span class="toggle"><input type="checkbox" id="log-autoscroll-toggle" :aria-label="LOG_AUTOSCROLL" :checked="autoscroll" @change="setAutoscroll(($event.target as HTMLInputElement).checked)"><span class="toggle-track"></span></span>
        {{ LOG_AUTOSCROLL }}
      </span>
      <button type="button" class="btn btn-sm" :class="{ active: paused }" id="log-pause-btn" @click="togglePause">{{ paused ? LOG_RESUME : LOG_PAUSE }}</button>
      <button type="button" class="btn btn-sm" id="log-download-btn" @click="download">{{ LOG_DOWNLOAD }}</button>
    </div>
    <p v-if="pollError" class="field-error-text" role="alert" id="log-poll-error">{{ pollError }}</p>
    <div ref="logWindowEl" class="log-window" id="logwin" role="log" aria-live="off" tabindex="0" :aria-label="LOG_WINDOW_LABEL" @scroll="onScroll">
      <div role="list">
        <template v-for="e in shown" :key="e.key">
          <div v-if="e.gap" class="log-gap hint" role="listitem">{{ GAP_TEXT }}</div>
          <div v-else class="log-line" role="listitem" :data-tag="e.tag"><span class="log-time">{{ formatLogTime(e.t) }}</span><span class="log-tag" :class="'tag-' + e.tag">[{{ e.tag }}]</span> {{ e.text }}</div>
        </template>
      </div>
    </div>
    <p class="hint" id="log-ring-hint">{{ hint }}</p>
  </div>
  <div v-else class="card"><p class="hint">{{ LOG_LOADING }}</p></div>
</template>
