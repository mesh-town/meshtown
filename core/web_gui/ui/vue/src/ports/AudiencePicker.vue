<script setup lang="ts">
import type { AudienceCandidate, AudienceMode } from "./ports-view.ts";
import { PORTS_AUDIENCE_LABEL, PORTS_AUDIENCE_PICK_ALL, PORTS_AUDIENCE_PICK_NODES, PORTS_NO_PEERS, PORTS_OFFLINE } from "./ports-texts.ts";

// "All members / Selected members" choice with a list of checkboxes; `idPrefix` sets the field ids (`<prefix>audience-all`…).
defineProps<{ idPrefix: string; candidates: AudienceCandidate[] }>();
const mode = defineModel<AudienceMode>("mode", { required: true });
const selected = defineModel<string[]>("selected", { required: true });
</script>

<template>
  <fieldset class="port-audience" :aria-label="PORTS_AUDIENCE_LABEL">
    <label class="exit-row"><input :id="idPrefix + 'audience-all'" v-model="mode" type="radio" :name="idPrefix + 'audience'" value="all"><span>{{ PORTS_AUDIENCE_PICK_ALL }}</span></label>
    <label class="exit-row"><input :id="idPrefix + 'audience-nodes'" v-model="mode" type="radio" :name="idPrefix + 'audience'" value="nodes"><span>{{ PORTS_AUDIENCE_PICK_NODES }}</span></label>
    <div v-if="mode === 'nodes'" :id="idPrefix + 'audience-list'">
      <label v-for="c in candidates" :key="c.idFull" class="port-audience-row" :title="c.idFull">
        <input v-model="selected" type="checkbox" :value="c.idFull" :data-port-audience-node="c.idFull">
        <span>{{ c.label }}</span>
        <span class="mono hint">{{ c.short }}</span>
        <span v-if="!c.online" class="hint">{{ PORTS_OFFLINE }}</span>
      </label>
      <p v-if="!candidates.length" class="hint">{{ PORTS_NO_PEERS }}</p>
    </div>
  </fieldset>
</template>
