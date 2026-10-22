<script setup lang="ts">
import { computed, ref } from "vue";
import type { PeerInfo, PublishedPortConfig } from "../core/api-types.ts";
import AudiencePicker from "./AudiencePicker.vue";
import { audienceCandidatesWith, audienceOf } from "./ports-view.ts";
import type { AudienceMode } from "./ports-view.ts";
import { PORTS_CANCEL, PORTS_NAME_PLACEHOLDER, PORTS_SAVE, portsEditTitleText } from "./ports-texts.ts";

// The editor of the name and "shared with" under a row of "My published ports"; the fields are filled with the current entry.
const props = defineProps<{
  port: number;
  entry: PublishedPortConfig | undefined;
  peers: PeerInfo[];
  busy: boolean;
  error: string;
  save: (port: number, name: unknown, audience: "all" | string[]) => Promise<boolean>;
}>();
const emit = defineEmits<{ (e: "cancel"): void; (e: "saved"): void }>();

const current = props.entry ? props.entry.for : "all";
const nameValue = ref<string>(props.entry ? props.entry.name : "");
const mode = ref<AudienceMode>(current === "all" ? "all" : "nodes");
const selected = ref<string[]>(current === "all" ? [] : [...current]);
const candidates = computed(() => audienceCandidatesWith(props.peers, current === "all" ? [] : current));
const prefix = "port-edit-" + props.port + "-";

async function submit(): Promise<void> {
  if (await props.save(props.port, nameValue.value, audienceOf(mode.value, selected.value, candidates.value))) emit("saved");
}
</script>

<template>
  <tr>
    <td colspan="5">
      <div class="port-edit">
        <h3>{{ portsEditTitleText(port) }}</h3>
        <div class="row-inline">
          <input :id="prefix + 'name'" v-model="nameValue" type="text" class="input" maxlength="64" :placeholder="PORTS_NAME_PLACEHOLDER" :data-port-edit-name="port">
        </div>
        <AudiencePicker v-model:mode="mode" v-model:selected="selected" :id-prefix="prefix" :candidates="candidates" />
        <p class="field-error-text" role="alert" :hidden="!error">{{ error }}</p>
        <div class="row-inline">
          <button type="button" class="btn btn-primary btn-sm" data-port-edit-save :disabled="busy" @click="submit">{{ PORTS_SAVE }}</button>
          <button type="button" class="btn btn-sm" data-port-edit-cancel :disabled="busy" @click="emit('cancel')">{{ PORTS_CANCEL }}</button>
        </div>
      </div>
    </td>
  </tr>
</template>
