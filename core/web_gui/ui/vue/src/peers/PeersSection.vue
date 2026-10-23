<script setup lang="ts">
// The "Members" section: the removal-vote banner, the member list (each row with its "⋯" menu), the line under the
// list, the groups of nodes that are no longer in / not in the member list, the confirmation dialog. A computer that
// is out of the network sees the "excluded" card instead. Without a member list (older node) it is the plain list.
import { computed, inject, ref, watch } from "vue";
import { membersOf, netName, stateOf, visibleSlots } from "../core/slots.ts";
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
import { confirmable, displayNames, excludedVariant, isMeExcluded, memberById, meId, partitionRows, removedLine } from "../members/members-view.ts";
import { pendingProposal, proposalKey, pruneHidden } from "../members/vote-hidden.ts";
import { useMembersActions } from "../members/use-members-actions.ts";
import NetTabHead from "./NetTabHead.vue";
import NetTabs from "./NetTabs.vue";
import PeerRow from "./PeerRow.vue";
import { joiningTabs, netTabs, netWho } from "./net-tabs-view.ts";
import { PEERS_LOADING, PEERS_MAX_NETS, PEERS_NETS_HINT_OK, PEERS_NO_STORE, peersNetsHint } from "./peers-texts.ts";
import { useNetsHint } from "./use-nets-hint.ts";
import { orderedPeers } from "./peers-view.ts";

const injected = inject(STORE_KEY);
if (!injected) throw new Error(PEERS_NO_STORE);
const store: Store = injected;
// The network on show: with several networks the chosen tab, otherwise the leading one.
const visible = computed(() => visibleSlots(store.nets, store.slots));
const multi = computed(() => visible.value.length >= 2);
const joining = computed(() => joiningTabs(store));
const showRow = computed(() => multi.value || joining.value.length > 0);
const addOnly = computed(() => showRow.value && store.peersTab === "add");
const slot = computed(() => (multi.value ? (store.peersTab && visible.value.includes(store.peersTab) ? store.peersTab : visible.value[0]) : store.lead));
const state = computed(() => stateOf(store, slot.value));
const members = computed(() => membersOf(store, slot.value));
const tabs = computed(() => netTabs(store));
const hint = useNetsHint(() => visible.value.length);
const { status, dialog, ask, pick, joinAnother } = useMembersActions(store, () => slot.value);
const excluded = computed(() => isMeExcluded(state.value, members.value));
const variant = computed(() => excludedVariant(members.value, meId(state.value)));
const net = computed(() => (state.value && state.value.network && state.value.network.label) || "");
const hidden = ref<string[]>([]); // "Not now": keys of the proposals hidden until they change or go away

const me = computed(() => meId(state.value));
const names = computed(() => displayNames(members.value));
const parts = computed(() => partitionRows(orderedPeers(state.value && state.value.peers), members.value, me.value));
// A peer the member list does not know is shown as a plain row: no menu, no badge.
const rows = computed(() => parts.value.active as PeerInfo[]);
const removedLines = computed(() => parts.value.removed.map((m) => removedLine(m, names.value)));
const chosen = computed(() => chosenExitIds(store));
const waiting = computed(() => confirmable(members.value, me.value));
const proposal = computed(() => pendingProposal(waiting.value, hidden.value));
watch(waiting, (list) => (hidden.value = pruneHidden(hidden.value, list)));
const later = (): void => {
  if (proposal.value) hidden.value = [...hidden.value, proposalKey(proposal.value)];
};
const meVotes = computed(() => (members.value && members.value.me_admission ? members.value.me_admission.votes : 0));
const meNeed = computed(() => (members.value && members.value.me_admission ? members.value.me_admission.need : (members.value ? members.value.need : 2)));
const memberOf = (p: PeerInfo) => memberById(members.value, p.id_full);

// Another tab: the proposals hidden on the old one mean nothing here; the line under the list is cleared
// unless the old tab is gone (a left network keeps its farewell line).
watch(slot, (now, before) => {
  hidden.value = [];
  if (before !== now && visible.value.includes(before)) status.value = { text: "", cls: "" };
});

// The "Use as exit" button makes this member the only exit.
async function exitVia(idFull: string): Promise<void> {
  status.value = await saveSingleExit(store, idFull);
}
</script>

<template>
  <p v-if="!state && !addOnly" class="hint">{{ PEERS_LOADING }}</p>
  <template v-else>
    <p v-if="multi && !hint.seen.value" class="hint nets-hint" id="peers-nets-hint">{{ peersNetsHint(visible.length) }} <button type="button" class="btn btn-small" id="peers-nets-hint-ok" @click="hint.dismiss">{{ PEERS_NETS_HINT_OK }}</button></p>
    <NetTabs v-if="showRow" :tabs="tabs" :active="addOnly ? 'add' : slot" :joining="joining" :full="visible.length >= PEERS_MAX_NETS" />
    <template v-if="addOnly"></template>
    <ExcludedCard v-else-if="excluded" :variant="variant" :net="net" @join="joinAnother" />
    <template v-else-if="!state || !state.network">
      <NoNetwork />
      <p class="field-status" :class="status.cls" id="peers-exit-status" role="status" :hidden="!status.text">{{ status.text }}</p>
    </template>
    <template v-else>
      <NetTabHead :key="slot" :slot="slot" :name="netName(store.nets, slot)" :who="netWho(store, slot)" :multi="showRow" />
      <PendingCard v-if="members && members.me_pending" :votes="meVotes" :need="meNeed" />
      <VoteBanner v-if="proposal && proposal.vote" :by="names[proposal.vote.proposed_by] || proposal.vote.proposed_by.slice(0, 8)" :target="names[proposal.id_full]" :reason="proposal.vote.reason" @confirm="ask('confirm', proposal.id_full)" @later="later" />
      <div class="card">
        <ul class="peer-list" id="peers-list" role="list">
          <PeerRow v-for="p in rows" :key="p.id_full || p.id" :peer="p" :state="state!" :chosen="chosen" :member="memberOf(p)" :me="me" :names="names" @exit-via="exitVia" @member-pick="pick" />
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
