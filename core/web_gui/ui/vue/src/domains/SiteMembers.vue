<script setup lang="ts">
// The rows of one site's details: this computer, then every member with the mark and its age.
// Plain rows, not table cells: the narrow card reuses it.
import { computed } from "vue";
import { agoText, peerLabel, EXIT_VIA_HIM } from "../common/words.ts";
import { DOMAINS_ROW_DIRECT, DOMAINS_NOT_CHECKED_CARD, DOMAINS_YOUR_EXIT, domainsAllSitesViaText } from "./domains-texts.ts";
import { domainCells } from "./domains-view.ts";
import type { MergedDomain, SiteNet } from "./domains-view.ts";

const props = defineProps<{ domain: MergedDomain; nets: SiteNet[]; proxy: string; current: string }>();
const emit = defineEmits<{ (e: "exit-via", idFull: string): void }>();

const rows = computed(() =>
  domainCells(props.domain, props.nets)
    .filter((c) => c.kind !== "empty")
    .map((c) => {
      const net = props.nets.find((n) => n.slot === c.slot);
      const label = c.peer ? peerLabel(c.peer, true, net?.peers || []) : DOMAINS_ROW_DIRECT;
      const mine = c.peer !== null && c.slot === props.proxy;
      const isCurrent = mine && c.peer !== null && c.peer.id_full === props.current;
      const canUse = mine && !isCurrent && c.peer !== null && c.peer.online && c.check?.verdict === "ok";
      const age = c.check ? agoText(c.check.age_sec) : "";
      return { ...c, label, age, isCurrent, canUse };
    }),
);
</script>

<template>
  <div class="site-members">
    <div v-for="r in rows" :key="r.key" class="site-member">
      <span class="site-member-who">{{ r.label }}<span v-if="r.tail" class="hint"> ({{ r.tail }})</span></span>
      <span v-if="!r.checked" class="hint">{{ DOMAINS_NOT_CHECKED_CARD }}</span>
      <span v-else class="site-member-mark"><span class="badge" :class="r.mark.cls" :title="r.mark.title">{{ r.mark.text }}</span><span v-if="r.age" class="hint"> · {{ r.age }}</span></span>
      <span v-if="r.isCurrent" class="peer-chosen">{{ DOMAINS_YOUR_EXIT }}</span>
      <button v-else-if="r.canUse && r.peer" type="button" class="btn btn-sm" :title="domainsAllSitesViaText(r.label)" @click="emit('exit-via', r.peer.id_full)">{{ EXIT_VIA_HIM }}</button>
    </div>
  </div>
</template>
