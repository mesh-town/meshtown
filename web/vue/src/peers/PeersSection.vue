<script setup lang="ts">
// The "Members" section: the removal-vote banner, the member list (each row with its "⋯" menu), the line under the
// list, the groups of nodes that are no longer in / not in the member list, the confirmation dialog. A computer that
// is out of the network sees the "excluded" card instead. Without a member list (older node) it is the plain list.
import { computed, inject, ref, watch } from "vue";
import { saveSingleExit } from "../core/exit-via.ts";
import { STORE_KEY } from "../core/store.ts";
import type { Store } from "../core/store.ts";
import type { PeerInfo } from "../core/api-types.ts";
import NoNetwork from "../common/NoNetwork.vue";
import { chosenExitIds } from "../common/exits.ts";
import ConfirmDialog from "../members/ConfirmDialog.vue";
import ExcludedCard from "../members/ExcludedCard.vue";
import PendingCard from "../members/PendingCard.vue";
import PendingList from "../members/PendingList.vue";
import RemovedList from "../members/RemovedList.vue";
import UnlistedList from "../members/UnlistedList.vue";
import VoteBanner from "../members/VoteBanner.vue";
import { confirmable, displayNames, memberById, meId, partitionRows, removedLine } from "../members/members-view.ts";
import { pendingProposal, proposalKey, pruneHidden } from "../members/vote-hidden.ts";
import { useExcluded } from "../members/use-excluded.ts";
import { useMembersActions } from "../members/use-members-actions.ts";
import PeerRow from "./PeerRow.vue";
import { PEERS_LOADING, PEERS_NO_STORE } from "./peers-texts.ts";
import { orderedPeers } from "./peers-view.ts";

const injected = inject(STORE_KEY);
if (!injected) throw new Error(PEERS_NO_STORE);
const store: Store = injected;
const { status, dialog, ask, pick, joinAnother } = useMembersActions(store);
const { excluded, variant, net } = useExcluded(store);
const hidden = ref<string[]>([]); // "Not now": keys of the proposals hidden until they change or go away

const me = computed(() => meId(store.state));
const names = computed(() => displayNames(store.members));
const parts = computed(() => partitionRows(orderedPeers(store.state && store.state.peers), store.members, me.value));
// A peer the member list does not know is shown as a plain row: no menu, no badge.
const rows = computed(() => parts.value.active as PeerInfo[]);
const removedLines = computed(() => parts.value.removed.map((m) => removedLine(m, names.value)));
const chosen = computed(() => chosenExitIds(store));
const waiting = computed(() => confirmable(store.members, me.value));
const proposal = computed(() => pendingProposal(waiting.value, hidden.value));
watch(waiting, (list) => (hidden.value = pruneHidden(hidden.value, list)));
const later = (): void => {
  if (proposal.value) hidden.value = [...hidden.value, proposalKey(proposal.value)];
};
const meVotes = computed(() => (store.members && store.members.me_admission ? store.members.me_admission.votes : 0));
const meNeed = computed(() => (store.members && store.members.me_admission ? store.members.me_admission.need : (store.members ? store.members.need : 2)));
const memberOf = (p: PeerInfo) => memberById(store.members, p.id_full);

// The "Use as exit" button makes this member the only exit.
async function exitVia(idFull: string): Promise<void> {
  status.value = await saveSingleExit(store, idFull);
}
</script>

<template>
  <p v-if="!store.state" class="hint">{{ PEERS_LOADING }}</p>
  <template v-else>
    <ExcludedCard v-if="excluded" :variant="variant" :net="net" @join="joinAnother" />
    <template v-else-if="!store.state.network">
      <NoNetwork />
      <p class="field-status" :class="status.cls" id="peers-exit-status" role="status" :hidden="!status.text">{{ status.text }}</p>
    </template>
    <template v-else>
      <PendingCard v-if="store.members && store.members.me_pending" :votes="meVotes" :need="meNeed" />
      <VoteBanner v-if="proposal && proposal.vote" :by="names[proposal.vote.proposed_by] || proposal.vote.proposed_by.slice(0, 8)" :target="names[proposal.id_full]" :reason="proposal.vote.reason" @confirm="ask('confirm', proposal.id_full)" @later="later" />
      <div class="card">
        <ul class="peer-list" id="peers-list" role="list">
          <PeerRow v-for="p in rows" :key="p.id_full || p.id" :peer="p" :state="store.state" :chosen="chosen" :member="memberOf(p)" :me="me" :names="names" @exit-via="exitVia" @member-pick="pick" />
        </ul>
        <p class="field-status" :class="status.cls" id="peers-exit-status" role="status" :hidden="!status.text">{{ status.text }}</p>
      </div>
      <PendingList :rows="parts.pending" :me="me" @pick="pick" />
      <RemovedList :lines="removedLines" />
      <UnlistedList :rows="parts.unlisted" @pick="pick" />
    </template>
    <ConfirmDialog ref="dialog" />
  </template>
</template>
