<script setup lang="ts">
import { COMMON_NO_NETWORK_AFTER, COMMON_NO_NETWORK_TEXT, COMMON_TAB_HOME_QUOTED } from "./common-texts.ts";
</script>

<template>
  <div class="card" data-no-network-msg="1">
    <p class="hint">{{ COMMON_NO_NETWORK_TEXT }} {{ COMMON_NO_NETWORK_AFTER }} <a href="#home">{{ COMMON_TAB_HOME_QUOTED }}</a> tab.</p>
  </div>
</template>
