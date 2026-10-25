<script setup lang="ts">
// The group under the member list: nodes that knocked but are not in the member list. They cannot be removed, only
// blocked / unblocked on this computer; the parent opens the dialog or calls the API on `pick`.
import { forDevice } from "../common/device.ts";
import MemberMenu from "./MemberMenu.vue";
import { BADGE_UNLISTED, MENU_BLOCK, MENU_UNBLOCK, unlistedSummary } from "./members-texts.ts";
import type { MenuItem } from "./members-types.ts";

interface UnlistedRow {
  id: string;
  id_full: string;
  name: string | null;
  blocked_by_me: boolean;
}

defineProps<{ rows: UnlistedRow[] }>();
defineEmits<{ (e: "pick", key: string, idFull: string): void }>();

const nameOf = (r: UnlistedRow): string => (r.name && r.name.trim() ? r.name : r.id);
const itemsOf = (r: UnlistedRow): MenuItem[] =>
  r.blocked_by_me ? [{ key: "unblock", label: MENU_UNBLOCK }] : [{ key: "block", label: forDevice(MENU_BLOCK) }];
</script>

<template>
  <details v-if="rows.length > 0" id="unlisted-list" class="card removed">
    <summary>{{ unlistedSummary(rows.length) }}</summary>
    <ul class="peer-list" role="list">
      <li v-for="r in rows" :key="r.id_full" class="peer-row">
        <span class="peer-who"><span class="peer-name">{{ nameOf(r) }}</span><span class="peer-badge badge-grey">{{ BADGE_UNLISTED }}</span><br><span class="peer-code mono">{{ r.id }}</span></span>
        <span class="peer-action"><MemberMenu :id="r.id_full" :name="nameOf(r)" :items="itemsOf(r)" @pick="(key) => $emit('pick', key, r.id_full)" /></span>
      </li>
    </ul>
  </details>
</template>
