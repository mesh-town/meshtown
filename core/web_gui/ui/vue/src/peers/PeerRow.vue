<script setup lang="ts">
// One row of the member list: the name and id, the connection, the main action and (with a member list) the
// removal-vote badge, extra lines and the "⋯" menu. Only reports what was chosen; the parent acts.
import { computed } from "vue";
import type { NodeState, PeerInfo } from "../core/api-types.ts";
import { EXIT_VIA_HIM, peerName } from "../common/words.ts";
import MemberMenu from "../members/MemberMenu.vue";
import { ACTION_CONFIRM, ACTION_UNBLOCK, confirmAria, unblockAria } from "../members/members-texts.ts";
import type { MemberRow } from "../members/members-types.ts";
import { menuItems, rowDecor } from "../members/members-view.ts";
import { PEERS_CHOSEN_DOWN, PEERS_CHOSEN_OK, PEERS_EDIT, PEERS_OLDER, PEERS_REFUSED, PEERS_YOU_PREFIX } from "./peers-texts.ts";
import { countryCell, olderMark, olderTitle, peerAction, peerMetaText, versionText } from "./peers-view.ts";

const props = defineProps<{ peer: PeerInfo; state: NodeState; chosen: string[]; member?: MemberRow | null; me?: string; names?: Record<string, string>; multi?: boolean }>();
const emit = defineEmits<{ (e: "exit-via", idFull: string): void; (e: "member-pick", key: string, idFull: string): void }>();

const decor = computed(() => rowDecor(props.peer, props.member, props.me || "", props.names || {}));
const items = computed(() => (props.member ? menuItems(props.member, { me: props.me || "" }) : []));
const action = computed(() => peerAction(props.peer, props.chosen, !!(props.member && props.member.blocked_by_me)));
const meta = computed(() => peerMetaText(props.peer, props.state));
const name = computed(() => peerName(props.peer));
const rowTitle = computed(() => (props.peer.self && props.state.node.id_full) || undefined);
</script>

<template>
  <li class="peer-row" :class="{ 'peer-self': peer.self }" :title="rowTitle">
    <span class="dot" :class="peer.online || peer.self ? 'dot-green' : 'dot-grey'" aria-hidden="true"></span>
    <span class="peer-who"><span class="peer-name">{{ peer.self ? PEERS_YOU_PREFIX : "" }}{{ name }}</span><span v-if="decor.badge" class="peer-badge" :class="'badge-' + decor.badge.tone">{{ decor.badge.text }}</span> <span v-if="countryCell(peer)" class="peer-country">{{ countryCell(peer) }}</span> <span v-if="versionText(peer)" class="peer-version">{{ versionText(peer) }}</span> <span v-if="olderMark(peer, state)" class="peer-older" :title="olderTitle(peer, state)">{{ PEERS_OLDER }}</span><br><span class="peer-code mono">{{ peer.id }}</span></span>
    <span class="peer-meta">{{ meta }}<span v-for="line in decor.lines" :key="line" class="peer-line">{{ line }}</span></span>
    <span class="peer-action">
      <a v-if="action.kind === 'self'" :href="multi ? '#' : '#home'" class="peer-note" @click="multi && ($event.preventDefault(), emit('member-pick', 'rename', peer.id_full))">{{ PEERS_EDIT }}</a>
      <span v-else-if="action.kind === 'chosen-ok'" class="peer-chosen">{{ PEERS_CHOSEN_OK }}</span>
      <span v-else-if="action.kind === 'chosen-down'" class="peer-chosen">{{ PEERS_CHOSEN_DOWN }}</span>
      <button v-else-if="action.kind === 'button'" type="button" class="btn btn-sm" :data-exit-via="peer.id_full" :aria-label="EXIT_VIA_HIM + ': ' + name" @click="emit('exit-via', peer.id_full)">{{ EXIT_VIA_HIM }}</button>
      <span v-else-if="action.kind === 'refused'" class="peer-note">{{ PEERS_REFUSED }}</span>
      <button v-if="decor.mainAction === 'confirm'" type="button" class="btn btn-sm btn-primary" data-member-action="confirm" :aria-label="confirmAria(name)" @click="emit('member-pick', 'confirm', peer.id_full)">{{ ACTION_CONFIRM }}</button>
      <button v-else-if="decor.mainAction === 'unblock'" type="button" class="btn btn-sm" data-member-action="unblock" :aria-label="unblockAria(name)" @click="emit('member-pick', 'unblock', peer.id_full)">{{ ACTION_UNBLOCK }}</button>
      <MemberMenu v-if="items.length > 0" :id="peer.id_full" :name="name" :items="items" @pick="(key) => emit('member-pick', key, peer.id_full)" />
    </span>
  </li>
</template>
