<script setup lang="ts">
// Sticky save bar; what to show is decided by `saveBarView` (`settings-view.ts`).
import { SETTINGS_DIRTY_HINT, SETTINGS_RESET, SETTINGS_RESTART_NODE, SETTINGS_SAVE } from "./settings-texts.ts";
import type { SaveBarView } from "./settings-view.ts";

withDefaults(defineProps<{ bar: SaveBarView; messageCls?: string }>(), { messageCls: "hint" });
defineEmits<{ (e: "save"): void; (e: "reset"): void; (e: "restart"): void }>();
</script>

<template>
  <div id="settings-save-bar" :class="bar.cls" :hidden="!bar.visible">
    <span class="hint" id="settings-dirty-hint" :hidden="!bar.dirtyHint">{{ SETTINGS_DIRTY_HINT }}</span>
    <div class="btn-row">
      <button type="button" class="btn btn-primary" id="settings-save-btn" :disabled="bar.saveDisabled" @click="$emit('save')">{{ SETTINGS_SAVE }}</button>
      <button type="button" class="btn" id="settings-reset-btn" @click="$emit('reset')">{{ SETTINGS_RESET }}</button>
      <span id="settings-save-message" role="status" :class="messageCls">{{ bar.messageShown }}</span>
      <button type="button" class="btn" id="restart-node-btn" :hidden="!bar.restartOffered" @click="$emit('restart')">{{ SETTINGS_RESTART_NODE }}</button>
    </div>
    <p class="hint" id="settings-writable-reason" :hidden="!bar.writableReason">{{ bar.writableReason }}</p>
  </div>
</template>
