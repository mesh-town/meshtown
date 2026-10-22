<script setup lang="ts">
import { computed, ref } from "vue";
import type { PeerInfo } from "../core/api-types.ts";
import { audienceCandidates, audienceOf } from "./ports-view.ts";
import type { AudienceMode } from "./ports-view.ts";
import AudiencePicker from "./AudiencePicker.vue";
import { PORTS_NAME_PLACEHOLDER, PORTS_PORT_PLACEHOLDER, PORTS_PUBLISH } from "./ports-texts.ts";

const props = defineProps<{
  busy: boolean;
  peers: PeerInfo[];
  publish: (port: unknown, name: unknown, audience: "all" | string[]) => Promise<boolean>;
}>();

const portValue = ref<string | number>("");
const nameValue = ref<string>("");
const mode = ref<AudienceMode>("all");
const selected = ref<string[]>([]);
const candidates = computed(() => audienceCandidates(props.peers));

async function add(): Promise<void> {
  if (!(await props.publish(portValue.value, nameValue.value, audienceOf(mode.value, selected.value, candidates.value)))) return;
  portValue.value = "";
  nameValue.value = "";
  mode.value = "all";
  selected.value = [];
}
</script>

<template>
  <div>
    <div class="row-inline">
      <input id="port-add-port" v-model="portValue" type="number" class="input input-port" min="1" max="65535" :placeholder="PORTS_PORT_PLACEHOLDER">
      <input id="port-add-name" v-model="nameValue" type="text" class="input" maxlength="64" :placeholder="PORTS_NAME_PLACEHOLDER">
      <button id="port-add-btn" type="button" class="btn btn-primary" :disabled="busy" @click="add">{{ PORTS_PUBLISH }}</button>
    </div>
    <AudiencePicker v-model:mode="mode" v-model:selected="selected" id-prefix="port-add-" :candidates="candidates" />
  </div>
</template>
