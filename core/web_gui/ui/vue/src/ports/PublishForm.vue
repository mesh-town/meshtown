<script setup lang="ts">
import { computed, ref } from "vue";
import type { PeerInfo, PortProto } from "../core/api-types.ts";
import { audienceCandidates, audienceOf } from "./ports-view.ts";
import type { AudienceMode } from "./ports-view.ts";
import type { PortAudience } from "./use-ports-actions.ts";
import AudiencePicker from "./AudiencePicker.vue";
import { PORTS_NAME_PLACEHOLDER, PORTS_PORT_PLACEHOLDER, PORTS_PROTO_LABEL, PORTS_PROTO_TCP, PORTS_PROTO_UDP, PORTS_PUBLISH } from "./ports-texts.ts";

const props = defineProps<{
  busy: boolean;
  peers: PeerInfo[];
  publish: (port: unknown, name: unknown, audience: PortAudience, proto: PortProto) => Promise<boolean>;
}>();

const portValue = ref<string | number>("");
const nameValue = ref<string>("");
const proto = ref<PortProto>("tcp");
const mode = ref<AudienceMode>("all");
const selected = ref<string[]>([]);
const candidates = computed(() => audienceCandidates(props.peers));

function audience(): PortAudience {
  return audienceOf(mode.value, selected.value, candidates.value);
}

async function add(): Promise<void> {
  if (!(await props.publish(portValue.value, nameValue.value, audience(), proto.value))) return;
  portValue.value = "";
  nameValue.value = "";
  proto.value = "tcp";
  mode.value = "all";
  selected.value = [];
}
</script>

<template>
  <div>
    <div class="row-inline">
      <input id="port-add-port" v-model="portValue" type="number" class="input input-port" min="1" max="65535" :placeholder="PORTS_PORT_PLACEHOLDER">
      <select id="port-add-proto" v-model="proto" class="input" :aria-label="PORTS_PROTO_LABEL">
        <option value="tcp">{{ PORTS_PROTO_TCP }}</option>
        <option value="udp">{{ PORTS_PROTO_UDP }}</option>
      </select>
      <input id="port-add-name" v-model="nameValue" type="text" class="input" maxlength="64" :placeholder="PORTS_NAME_PLACEHOLDER">
      <button id="port-add-btn" type="button" class="btn btn-primary" :disabled="busy" @click="add">{{ PORTS_PUBLISH }}</button>
    </div>
    <AudiencePicker v-model:mode="mode" v-model:selected="selected" id-prefix="port-add-" :candidates="candidates" />
  </div>
</template>
