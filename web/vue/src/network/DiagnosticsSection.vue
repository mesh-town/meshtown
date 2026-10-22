<script setup lang="ts">
import { inject } from "vue";
import { STORE_KEY } from "../core/store.ts";
import DiagChecks from "./DiagChecks.vue";
import DiagSpecialist from "./DiagSpecialist.vue";
import {
  DIAG_EVENTS_EMPTY, DIAG_EVENTS_TITLE, DIAG_FULL_LOG_LINK, DIAG_LOADING, DIAG_STATE_TITLE, DIAG_STORE_MISSING, DIAG_SUMMARY_BTN,
  DIAG_SUMMARY_LABEL,
} from "./diag-texts.ts";
import { useDiagnostics } from "./use-diagnostics.ts";

const store = inject(STORE_KEY);
if (!store) throw new Error(DIAG_STORE_MISSING);
const d = useDiagnostics(store);
const { form, checks, events, copySummary } = d;
</script>

<template>
  <div id="diagnostics-root">
    <div v-if="store.state === null" class="card"><p class="hint">{{ DIAG_LOADING }}</p></div>
    <template v-else>
      <div class="card">
        <h2>{{ DIAG_STATE_TITLE }}</h2>
        <DiagChecks :checks="checks" />
      </div>
      <div class="card">
        <h2>{{ DIAG_EVENTS_TITLE }}</h2>
        <ul v-if="events.length" class="diag-list" role="list" :aria-label="DIAG_EVENTS_TITLE">
          <li v-for="e in events" :key="e.seq" class="event-row">
            <span class="mono">{{ e.time }}</span> <span class="badge">{{ e.tag }}</span> <span>{{ e.text }}</span>
          </li>
        </ul>
        <p v-else class="hint" role="status">{{ DIAG_EVENTS_EMPTY }}</p>
        <p class="hint"><a href="#log">{{ DIAG_FULL_LOG_LINK }}</a></p>
      </div>
      <div class="card">
        <div class="btn-row">
          <button type="button" class="btn" id="diag-summary-btn" :disabled="form.summary.busy" @click="copySummary">{{ DIAG_SUMMARY_BTN }}</button>
        </div>
        <p v-if="form.summary.status" class="hint" role="status">{{ form.summary.status }}</p>
        <textarea v-if="form.summary.fallback" class="mono" readonly :aria-label="DIAG_SUMMARY_LABEL" :value="form.summary.fallback"></textarea>
      </div>
      <DiagSpecialist :diag="d" />
    </template>
  </div>
</template>
