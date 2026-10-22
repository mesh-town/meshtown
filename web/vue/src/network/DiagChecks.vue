<script setup lang="ts">
import type { CheckRow } from "./diag-checks.ts";
import { LEVEL_ICON } from "./diag-checks.ts";
import { DIAG_CHECKS_LABEL } from "./diag-texts.ts";

defineProps<{ checks: CheckRow[] }>();
</script>

<template>
  <ul class="diag-list" :aria-label="DIAG_CHECKS_LABEL">
    <li v-for="c in checks" :key="c.key" class="diag-row">
      <span :class="['diag-icon', c.level]" aria-hidden="true">{{ LEVEL_ICON[c.level] }}</span>
      <a v-if="c.href" :href="c.href">{{ c.text }}</a>
      <span v-else>{{ c.text }}</span>
    </li>
  </ul>
</template>
