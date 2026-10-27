<script setup lang="ts">
// Collapsed extras under the setup steps: strict mode and the manual SOCKS5 block.
import ManualSetup from "../ManualSetup.vue";
import StrictChoice from "../StrictChoice.vue";
import { HOME_WIZARD_MORE } from "../home-card-texts.ts";
</script>

<template>
  <details id="more-options" class="more-options">
    <summary>{{ HOME_WIZARD_MORE }}</summary>
    <StrictChoice />
    <ManualSetup wizard />
  </details>
</template>
