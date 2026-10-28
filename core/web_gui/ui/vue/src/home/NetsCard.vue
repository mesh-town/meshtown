<script setup lang="ts">
// The network lines of "Home" with two or more networks: each line is one link (dot, name, member summary, chevron) to that network's members tab.
import { inject } from "vue";
import { STORE_KEY } from "../core/store.ts";
import { peersHash } from "../shell/nav.ts";
import { NETS_CARD_TITLE } from "./nets-texts.ts";
import { useHomeContext } from "./use-home.ts";

const { netRows } = useHomeContext(inject(STORE_KEY)!);
</script>

<template>
  <div class="card card-full" id="home-nets" role="region" aria-labelledby="home-nets-title">
    <h2 id="home-nets-title">{{ NETS_CARD_TITLE }}</h2>
    <a v-for="row in netRows" :key="row.slot" class="home-net-row" :href="peersHash(row.slot)" :data-slot="row.slot">
      <span class="state-dot" :class="row.dot" aria-hidden="true"></span>
      <strong class="net-row-title">{{ row.title }}</strong>
      <span class="hint net-row-summary">— {{ row.summary }}</span>
      <span class="hint home-net-row-chevron" aria-hidden="true">›</span>
    </a>
  </div>
</template>
