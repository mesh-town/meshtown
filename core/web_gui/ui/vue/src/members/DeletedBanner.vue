<script setup lang="ts">
// The line after deleting a network for everyone; shown on Members and in Settings.
import { onUnmounted } from "vue";
import { DELETED_OK, deletedBannerText, useDeletedBanner } from "./deleted-banner.ts";

const { deleted, clear } = useDeletedBanner();
onUnmounted(clear);
</script>

<template>
  <p v-if="deleted !== null" class="hint net-banner" id="deleted-net-banner" role="status">{{ deletedBannerText(deleted) }} <button type="button" class="btn btn-small" id="deleted-net-ok" @click="clear">{{ DELETED_OK }}</button></p>
</template>
