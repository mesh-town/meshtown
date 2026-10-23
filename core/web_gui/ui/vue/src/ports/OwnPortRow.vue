<script setup lang="ts">
import type { OwnPort, PeerInfo, PublishedPortConfig } from "../core/api-types.ts";
import type { NetGroup } from "./ports-view.ts";
import type { PortAudience } from "./use-ports-actions.ts";
import PortEditRow from "./PortEditRow.vue";
import { portStatusText } from "./ports-view.ts";
import { PORTS_EDIT, PORTS_UNPUBLISH } from "./ports-texts.ts";

// A row of "My published ports"; while `editing`, the editor is expanded under it.
defineProps<{
  row: Pick<OwnPort, "port" | "name" | "local">;
  entry: PublishedPortConfig | undefined;
  audience: string;
  peers: PeerInfo[];
  busy: boolean;
  editing: boolean;
  editError: string;
  groups?: NetGroup[];
  shared?: Record<string, "all" | string[]>;
  save: (port: number, name: unknown, audience: PortAudience) => Promise<boolean>;
}>();
const emit = defineEmits<{ (e: "edit"): void; (e: "cancel"): void; (e: "saved"): void; (e: "unpublish"): void }>();
</script>

<template>
  <tr>
    <td class="mono">{{ row.port }}</td>
    <td>{{ row.name }}</td>
    <td>{{ audience }}</td>
    <td>{{ portStatusText(row) }}</td>
    <td>
      <button type="button" class="btn btn-sm" :data-port-edit="row.port" :disabled="busy" @click="emit('edit')">{{ PORTS_EDIT }}</button>
      <button type="button" class="btn btn-sm" :data-port-unpublish="row.port" :disabled="busy" @click="emit('unpublish')">{{ PORTS_UNPUBLISH }}</button>
    </td>
  </tr>
  <PortEditRow v-if="editing" :port="row.port" :entry="entry" :peers="peers" :busy="busy" :error="editError" :groups="groups" :shared="shared" :save="save" @cancel="emit('cancel')" @saved="emit('saved')" />
</template>
