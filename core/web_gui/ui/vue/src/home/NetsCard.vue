<script setup lang="ts">
// The network tiles of "Home" with two or more networks: each tile is one link (the network's ring, its name, a hint) to that network's members tab.
import { inject } from "vue";
import { STORE_KEY } from "../core/store.ts";
import { peersHash } from "../shell/nav.ts";
import { NETS_CARD_TITLE } from "./nets-texts.ts";
import RingMark from "./RingMark.vue";
import { useRingSize } from "./ring-view.ts";
import { useHomeContext } from "./use-home.ts";

const { netRows } = useHomeContext(inject(STORE_KEY)!);
const dims = useRingSize();
</script>

<template>
  <div class="card card-full" id="home-nets" role="region" aria-labelledby="home-nets-title">
    <h2 id="home-nets-title">{{ NETS_CARD_TITLE }}</h2>
    <a v-for="row in netRows" :key="row.slot" class="home-net-row" :href="peersHash(row.slot)" :data-slot="row.slot">
      <RingMark :members="row.ring.members" :size="dims.size" :thickness="dims.thickness" :center="row.ring.center" />
      <strong class="net-row-title">{{ row.title }}</strong>
      <span v-if="row.hint" class="hint net-row-summary">{{ row.hint }}</span>
    </a>
  </div>
</template>
