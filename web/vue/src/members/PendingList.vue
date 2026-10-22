<script setup lang="ts">
// The card with join requests: nodes that asked to join and wait for confirmations. Shows the label of the invite and the
// count; reports `pick("admit" | "unadmit" | "block" | "unblock", id)` and does nothing else (the parent asks and calls the API).
import MemberMenu from "./MemberMenu.vue";
import { ACTION_ADMIT, BADGE_PENDING, PENDING_TITLE, admissionLine, admitAria, labelLine } from "./members-texts.ts";
import type { MemberRow } from "./members-types.ts";
import { canAdmit, menuItems } from "./members-view.ts";

const props = defineProps<{ rows: MemberRow[]; me: string }>();
defineEmits<{ (e: "pick", key: string, idFull: string): void }>();

const nameOf = (r: MemberRow): string => (r.name && r.name.trim() ? r.name : r.admission && r.admission.label ? r.admission.label : r.id);
const hasLabelLine = (r: MemberRow): boolean => !!(r.name && r.name.trim() && r.admission && r.admission.label);
const itemsOf = (r: MemberRow) => menuItems(r, { me: props.me });
</script>

<template>
  <section v-if="rows.length > 0" id="members-pending-list" class="card" role="region" aria-labelledby="members-pending-title">
    <h2 id="members-pending-title">{{ PENDING_TITLE }}</h2>
    <ul class="peer-list" role="list">
      <li v-for="r in rows" :key="r.id_full" class="peer-row" :data-pending="r.id_full">
        <span class="dot" :class="r.online ? 'dot-green' : 'dot-grey'" aria-hidden="true"></span>
        <span class="peer-who"><span class="peer-name">{{ nameOf(r) }}</span><span class="peer-badge badge-warn">{{ BADGE_PENDING }}</span><br><span class="peer-code mono">{{ r.id }}</span></span>
        <span class="peer-meta"><span v-if="r.admission" class="peer-line">{{ admissionLine(r.admission.votes, r.admission.need) }}</span><span v-if="hasLabelLine(r)" class="peer-line">{{ labelLine(r.admission!.label!) }}</span></span>
        <span class="peer-action">
          <button v-if="canAdmit(r, me)" type="button" class="btn btn-sm btn-primary" data-member-action="admit" :aria-label="admitAria(nameOf(r))" @click="$emit('pick', 'admit', r.id_full)">{{ ACTION_ADMIT }}</button>
          <MemberMenu v-if="itemsOf(r).length > 0" :id="r.id_full" :name="nameOf(r)" :items="itemsOf(r)" @pick="(key) => $emit('pick', key, r.id_full)" />
        </span>
      </li>
    </ul>
  </section>
</template>
