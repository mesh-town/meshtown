<script setup lang="ts">
// The collapsed group under the member list: nodes removed from the member list (or with a removal waiting for a second
// vote). Each row says who removed it and when; the "⋯" menu can bring it back. The parent handles `pick`.
import MemberMenu from "./MemberMenu.vue";
import { forgottenSummary } from "./members-texts.ts";
import type { MemberRow } from "./members-types.ts";
import { forgottenRowLine, menuItems } from "./members-view.ts";

const props = defineProps<{ rows: MemberRow[]; all: MemberRow[]; me: string }>();
defineEmits<{ (e: "pick", key: string, idFull: string): void }>();

const nameOf = (r: MemberRow): string => (r.name && r.name.trim() ? r.name : r.id);
const itemsOf = (r: MemberRow) => menuItems(r, { me: props.me });
</script>

<template>
  <details v-if="rows.length > 0" id="forgotten-list" class="card removed">
    <summary>{{ forgottenSummary(rows.length) }}</summary>
    <ul class="peer-list" role="list">
      <li v-for="r in rows" :key="r.id_full" class="peer-row" :data-forgotten="r.id_full">
        <span class="peer-who"><span class="peer-name">{{ nameOf(r) }}</span><br><span class="peer-code mono">{{ r.id }}</span></span>
        <span class="peer-meta"><span class="peer-line">{{ forgottenRowLine(r, props.all) }}</span></span>
        <span class="peer-action"><MemberMenu v-if="itemsOf(r).length > 0" :id="r.id_full" :name="nameOf(r)" :items="itemsOf(r)" @pick="(key) => $emit('pick', key, r.id_full)" /></span>
      </li>
    </ul>
  </details>
</template>
