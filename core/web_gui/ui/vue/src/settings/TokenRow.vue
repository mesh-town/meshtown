<script setup lang="ts">
// An access token row: show/hide and revoke.
import { settingsTokenDescText, SETTINGS_TOKEN_HIDE, SETTINGS_TOKEN_REVOKE, SETTINGS_TOKEN_SHOW, SETTINGS_TOKEN_TITLE } from "./settings-texts.ts";

withDefaults(defineProps<{ revealed?: boolean; token?: string }>(), { revealed: false, token: "" });
defineEmits<{ (e: "toggle"): void; (e: "rotate"): void }>();
</script>

<template>
  <div class="setting-row">
    <div class="setting-text">
      <div class="setting-title">{{ SETTINGS_TOKEN_TITLE }}</div>
      <div class="setting-desc">{{ settingsTokenDescText() }}</div>
    </div>
    <div class="setting-control">
      <code class="mono" id="token-display">{{ revealed ? token : "••••••••" }}</code>
      <button type="button" class="btn btn-sm" id="token-show-btn" @click="$emit('toggle')">{{ revealed ? SETTINGS_TOKEN_HIDE : SETTINGS_TOKEN_SHOW }}</button>
      <button type="button" class="btn btn-sm btn-danger" id="token-rotate-btn" @click="$emit('rotate')">{{ SETTINGS_TOKEN_REVOKE }}</button>
    </div>
  </div>
</template>
