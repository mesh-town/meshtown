<script setup lang="ts">
// The row of network tabs: one link per network (it keeps the open sub-tab), the networks being joined, and "+" (the add-network tab).
import { peersHash } from "../shell/nav.ts";
import type { NetSub } from "../shell/net-subs.ts";
import type { NetTab } from "./net-tabs-view.ts";
import { PEERS_TAB_ADD, PEERS_TAB_ADD_FULL, PEERS_TAB_ADD_MARK, PEERS_TAB_JOINING, PEERS_TAB_JOINING_SUB, peersVotesTitle } from "./peers-texts.ts";

defineProps<{ tabs: NetTab[]; active: string; joining: { name: string }[]; full: boolean; sub: NetSub }>();

// Networks with the same name carry " · xxxx"; the tail is drawn quieter.
const split = (name: string): [string, string] => {
  const at = name.lastIndexOf(" · ");
  return at < 0 ? [name, ""] : [name.slice(0, at), name.slice(at)];
};
</script>

<template>
  <nav class="ntabs" id="peers-net-tabs" aria-label="Networks">
    <a v-for="t in tabs" :key="t.slot" class="ntab" :class="{ active: active === t.slot }" :href="peersHash(t.slot, sub)" :aria-current="active === t.slot ? 'page' : undefined" :title="t.title" :data-slot="t.slot">
      <span class="lbl">{{ split(t.name)[0] }}</span><span v-if="split(t.name)[1]" class="tail">{{ split(t.name)[1] }}</span><span class="sub">{{ t.sub }}</span><span v-if="t.votes > 0" class="cnt" :title="peersVotesTitle(t.votes)">{{ t.votes }}</span>
    </a>
    <span v-for="(j, i) in joining" :key="'joining-' + i" class="ntab dis" aria-disabled="true" :title="PEERS_TAB_JOINING"><span class="lbl">{{ j.name }}</span><span class="sub">{{ PEERS_TAB_JOINING_SUB }}</span></span>
    <a class="ntab add" :class="{ active: active === 'add' }" :href="peersHash('add')" :aria-current="active === 'add' ? 'page' : undefined" :aria-label="full ? PEERS_TAB_ADD_FULL : PEERS_TAB_ADD" :title="full ? PEERS_TAB_ADD_FULL : PEERS_TAB_ADD">{{ PEERS_TAB_ADD_MARK }}</a>
  </nav>
</template>
