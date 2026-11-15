<script setup lang="ts">
// The yellow strip under the header while this computer's own exit is turning members away; on every tab.
import { computed, inject } from "vue";
import { STORE_KEY } from "../core/store.ts";
import { OWN_OVERLOAD_STRIP, OWN_OVERLOAD_STRIP_LINK, ownOverloadTarget } from "../peers/own-overload.ts";
import { SHELL_NO_STORE } from "./shell-texts.ts";

const store = inject(STORE_KEY);
if (!store) throw new Error(SHELL_NO_STORE);
const href = computed(() => ownOverloadTarget(store));
</script>

<template>
  <p v-if="href" id="overload-strip" class="warn-strip" role="status">{{ OWN_OVERLOAD_STRIP }} <a id="overload-strip-link" :href="href">{{ OWN_OVERLOAD_STRIP_LINK }}</a></p>
</template>
