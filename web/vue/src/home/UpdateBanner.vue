<script setup lang="ts">
// "A newer version is out": shown above the cards when GitHub or a member reports a higher version than this node's.
// The Android app links straight to the APK; the package-manager hint does not apply there.
import { computed, inject, toRef } from "vue";
import { STORE_KEY } from "../core/store.ts";
import {
  UPDATE_ANDROID_DOWNLOAD_URL, UPDATE_DISMISS, UPDATE_DOWNLOAD, UPDATE_PACKAGES_HINT, UPDATE_DOWNLOAD_URL, updateAvailableText,
} from "./update-texts.ts";
import { useUpdateCheck } from "./use-update-check.ts";

const store = inject(STORE_KEY)!;
const { own, newer, visible, dismiss } = useUpdateCheck(toRef(store, "state"));
const android = /Android/.test(navigator.userAgent);
const href = android ? UPDATE_ANDROID_DOWNLOAD_URL : UPDATE_DOWNLOAD_URL;
const text = computed(() => (newer.value && own.value ? updateAvailableText(newer.value, own.value) : ""));
</script>

<template>
  <div v-if="visible" class="card" id="update-banner" role="status">
    <p>{{ text }}</p>
    <p v-if="!android" class="hint" id="update-hint">{{ UPDATE_PACKAGES_HINT }}</p>
    <div class="hero-row">
      <a class="btn" id="update-download" :href="href" target="_blank" rel="noopener">{{ UPDATE_DOWNLOAD }}</a>
      <button type="button" class="btn" id="update-dismiss" @click="dismiss">{{ UPDATE_DISMISS }}</button>
    </div>
  </div>
</template>
