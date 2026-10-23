<script setup lang="ts">
import { watchEffect } from "vue";
import type { NetGroup, NetSelection } from "./ports-view.ts";
import { PORTS_AUDIENCE_LABEL, PORTS_AUDIENCE_PICK_ALL, PORTS_NO_PEERS, PORTS_OFFLINE, portsNetGroupText } from "./ports-texts.ts";

// One group per network: "All members" and the members of that network. Nothing checked in a group — not shared there.
const props = defineProps<{ idPrefix: string; groups: NetGroup[] }>();
const selection = defineModel<Record<string, NetSelection>>("selection", { required: true });

// Every group has its entry, so the checkboxes can bind to it.
watchEffect(() => {
  for (const g of props.groups) if (!selection.value[g.slot]) selection.value[g.slot] = { all: false, ids: [] };
});
</script>

<template>
  <fieldset class="port-audience" :aria-label="PORTS_AUDIENCE_LABEL">
    <div v-for="g in groups" :key="g.slot" :data-port-net-group="g.slot">
      <div class="hint">{{ portsNetGroupText(g.label) }}</div>
      <template v-if="selection[g.slot]">
        <label class="exit-row">
          <input :id="idPrefix + g.slot + '-all'" v-model="selection[g.slot].all" type="checkbox" :data-port-audience-all="g.slot"><span>{{ PORTS_AUDIENCE_PICK_ALL }}</span>
        </label>
        <label v-for="c in g.candidates" :key="c.idFull" class="port-audience-row" :title="c.idFull">
          <input v-model="selection[g.slot].ids" type="checkbox" :value="c.idFull" :disabled="selection[g.slot].all" :data-port-audience-node="c.idFull" :data-port-audience-net="g.slot">
          <span>{{ c.label }}</span>
          <span class="mono hint">{{ c.short }}</span>
          <span v-if="!c.online" class="hint">{{ PORTS_OFFLINE }}</span>
        </label>
        <p v-if="!g.candidates.length" class="hint">{{ PORTS_NO_PEERS }}</p>
      </template>
    </div>
  </fieldset>
</template>
