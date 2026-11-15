<script setup lang="ts">
// Collapsed extras under the setup steps: the manual SOCKS5 block.
import ManualSetup from "../ManualSetup.vue";
import { HOME_WIZARD_MORE } from "../home-card-texts.ts";
</script>

<template>
  <details id="more-options" class="more-options">
    <summary>{{ HOME_WIZARD_MORE }}</summary>
    <ManualSetup wizard />
  </details>
</template>
