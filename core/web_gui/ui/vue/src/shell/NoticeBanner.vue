<script setup lang="ts">
// The red strip of notices over Home and Members: one line per network an admin deleted, each with an OK that
// acknowledges it. The notices ride on the overview (`store.notices`).
import { computed, inject } from "vue";
import { STORE_KEY } from "../core/store.ts";
import { NOTICE_OK, noticeText, sortedNotices } from "./notice-texts.ts";
import { ackNotice } from "./use-notices.ts";
import { SHELL_NO_STORE } from "./shell-texts.ts";

const store = inject(STORE_KEY);
if (!store) throw new Error(SHELL_NO_STORE);
const notices = computed(() => sortedNotices(store.notices));
</script>

<template>
  <div v-if="notices.length > 0" id="notice-banner" class="warn-strip notice-banner" role="alert">
    <p v-for="n in notices" :key="n.id" class="notice-line">{{ noticeText(n) }} <button type="button" class="btn btn-small" id="notice-ok" @click="ackNotice(store, n.id)">{{ NOTICE_OK }}</button></p>
  </div>
</template>
