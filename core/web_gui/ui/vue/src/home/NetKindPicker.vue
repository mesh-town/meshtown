<script setup lang="ts">
// The choice of the kind of a network that is about to be created: two radio cards, the hint that the choice is final and,
// while Managed is chosen, the warning about the first admin's data. `v-model` is the kind; `name` — what is typed in the name field.
import type { NetKind } from "../core/api-types.ts";
import {
  KIND_FINAL_HINT, KIND_MANAGED_TEXT, KIND_MANAGED_TITLE, KIND_PEER_TEXT, KIND_PEER_TITLE, KIND_TITLE, managedWarning,
} from "./net-kind-texts.ts";

defineProps<{ modelValue: NetKind; name?: string }>();
const emit = defineEmits<{ "update:modelValue": [kind: NetKind] }>();
</script>

<template>
  <div class="kind-picker">
    <div class="field-label" id="kind-title">{{ KIND_TITLE }}</div>
    <div class="kinds" role="radiogroup" aria-labelledby="kind-title">
      <label class="kcard" :class="{ on: modelValue === 'Peer' }">
        <input type="radio" id="kind-peer" name="net-kind" value="Peer" :checked="modelValue === 'Peer'" @change="emit('update:modelValue', 'Peer')">
        <span class="rd" aria-hidden="true"></span>
        <span><b>{{ KIND_PEER_TITLE }}</b><span class="kt">{{ KIND_PEER_TEXT }}</span></span>
      </label>
      <label class="kcard" :class="{ on: modelValue === 'Managed' }">
        <input type="radio" id="kind-managed" name="net-kind" value="Managed" :checked="modelValue === 'Managed'" @change="emit('update:modelValue', 'Managed')">
        <span class="rd" aria-hidden="true"></span>
        <span><b>{{ KIND_MANAGED_TITLE }}</b><span class="kt">{{ KIND_MANAGED_TEXT }}</span></span>
      </label>
    </div>
    <p class="hint" id="kind-final-hint">{{ KIND_FINAL_HINT }}</p>
    <p v-if="modelValue === 'Managed'" class="hint kind-warn" id="kind-managed-warn">{{ managedWarning(name ?? "") }}</p>
  </div>
</template>
