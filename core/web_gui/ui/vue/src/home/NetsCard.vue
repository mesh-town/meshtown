<script setup lang="ts">
// The network lines of "Home" with two or more networks: a dot, the name, the member summary and the way to the members tab.
import { inject } from "vue";
import { STORE_KEY } from "../core/store.ts";
import { peersHash } from "../shell/nav.ts";
import { NETS_CARD_ARIA, NETS_OPEN_MEMBERS } from "./nets-texts.ts";
import { useHomeContext } from "./use-home.ts";

const { netRows } = useHomeContext(inject(STORE_KEY)!);
</script>

<template>
  <div class="card card-full" id="home-nets" role="region" :aria-label="NETS_CARD_ARIA">
    <div v-for="row in netRows" :key="row.slot" class="net-row" :data-slot="row.slot">
      <span class="state-dot" :class="row.dot" aria-hidden="true"></span>
      <strong class="net-row-title">{{ row.title }}</strong>
      <span class="hint net-row-summary">— {{ row.summary }}</span>
      <a class="btn btn-sm net-row-open" :href="peersHash(row.slot)">{{ NETS_OPEN_MEMBERS }}</a>
    </div>
  </div>
</template>
