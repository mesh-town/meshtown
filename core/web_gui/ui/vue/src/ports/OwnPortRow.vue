<script setup lang="ts">
import { computed } from "vue";
import type { OwnPort, PeerInfo, PortProto, PublishedPortConfig } from "../core/api-types.ts";
import type { PortAudience } from "./use-ports-actions.ts";
import PortEditRow from "./PortEditRow.vue";
import { portStatusText } from "./ports-view.ts";
import { portKey } from "./ports-proto.ts";
import { PORTS_EDIT, PORTS_PROTO_UDP, PORTS_UNPUBLISH } from "./ports-texts.ts";

// A row of "My published ports"; while `editing`, the editor is expanded under it.
const props = withDefaults(defineProps<{
  row: Pick<OwnPort, "port" | "name" | "local">;
  proto?: PortProto;
  entry: PublishedPortConfig | undefined;
  audience: string;
  peers: PeerInfo[];
  busy: boolean;
  editing: boolean;
  editError: string;
  save: (port: number, name: unknown, audience: PortAudience, proto: PortProto) => Promise<boolean>;
}>(), { proto: "tcp" });
const key = computed(() => portKey(props.proto, props.row.port));
const emit = defineEmits<{ (e: "edit"): void; (e: "cancel"): void; (e: "saved"): void; (e: "unpublish"): void }>();
</script>

<template>
  <tr>
    <td><span class="mono">{{ row.port }}</span><template v-if="proto === 'udp'"> <span class="badge badge-grey" data-port-proto="udp">{{ PORTS_PROTO_UDP }}</span></template></td>
    <td>{{ row.name }}</td>
    <td>{{ audience }}</td>
    <td>{{ portStatusText({ local: row.local, proto }) }}</td>
    <td>
      <button type="button" class="btn btn-sm" :data-port-edit="key" :disabled="busy" @click="emit('edit')">{{ PORTS_EDIT }}</button>
      <button type="button" class="btn btn-sm" :data-port-unpublish="key" :disabled="busy" @click="emit('unpublish')">{{ PORTS_UNPUBLISH }}</button>
    </td>
  </tr>
  <PortEditRow v-if="editing" :port="row.port" :proto="proto" :entry="entry" :peers="peers" :busy="busy" :error="editError" :save="save" @cancel="emit('cancel')" @saved="emit('saved')" />
</template>
