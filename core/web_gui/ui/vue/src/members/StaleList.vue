<script setup lang="ts">
// The collapsed group under the member list: members that have been offline for over a week (or never seen).
// Each row has its own "⋯" menu; there is no bulk action. The parent opens the dialog or calls the API on `pick`.
import MemberMenu from "./MemberMenu.vue";
import { BADGE_FORGET_PROPOSED, lastOnlineText, staleSummary } from "./members-texts.ts";
import type { MemberRow } from "./members-types.ts";
import { menuItems } from "./members-view.ts";

const props = defineProps<{ rows: MemberRow[]; me: string }>();
defineEmits<{ (e: "pick", key: string, idFull: string): void }>();

const nameOf = (r: MemberRow): string => (r.name && r.name.trim() ? r.name : r.id);
const itemsOf = (r: MemberRow) => menuItems(r, { me: props.me });
</script>

<template>
  <details v-if="rows.length > 0" id="stale-list" class="card stale">
    <summary>{{ staleSummary(rows.length) }}</summary>
    <ul class="peer-list" role="list">
      <li v-for="r in rows" :key="r.id_full" class="peer-row" :data-stale="r.id_full">
        <span class="dot dot-grey" aria-hidden="true"></span>
        <span class="peer-who"><span class="peer-name">{{ nameOf(r) }}</span><span v-if="r.forget" class="peer-badge badge-warn">{{ BADGE_FORGET_PROPOSED }}</span><br><span class="peer-code mono">{{ r.id }}</span></span>
        <span class="peer-meta"><span class="peer-line">{{ lastOnlineText(r.last_seen_sec) }}</span></span>
        <span class="peer-action"><MemberMenu v-if="itemsOf(r).length > 0" :id="r.id_full" :name="nameOf(r)" :items="itemsOf(r)" @pick="(key) => $emit('pick', key, r.id_full)" /></span>
      </li>
    </ul>
  </details>
</template>
