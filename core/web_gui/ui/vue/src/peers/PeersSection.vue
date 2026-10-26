<script setup lang="ts">
// The "Members" section: the second-admin hint, the member list (each row with its "⋯" menu), the line under the
// list, the groups of nodes that are not in / no longer in the member list, the confirmation dialog. A computer that
// is out of the network sees the "excluded" card instead. Without a member list (older node) it is the plain list.
import { computed, inject, nextTick, ref, watch } from "vue";
import { isRefused, membersOf, netName, peersSlots, refusedFlag, stateOf } from "../core/slots.ts";
import { saveExitIn } from "../core/exit-via.ts";
import { STORE_KEY } from "../core/store.ts";
import type { Store } from "../core/store.ts";
import type { PeerInfo } from "../core/api-types.ts";
import JoiningCard from "../home/JoiningCard.vue";
import { refusedLead, refusedText } from "../home/join-wait.ts";
import { useRefusalGate } from "../home/use-refusal-gate.ts";
import { REFUSED_LEAD } from "../home/invite-join-texts.ts";
import { quotedNet } from "../common/words.ts";
import NoNetwork from "../common/NoNetwork.vue";
import { chosenOf } from "../home/exit-groups.ts";
import { chosenExitIds } from "../common/exits.ts";
import ConfirmDialog from "../members/ConfirmDialog.vue";
import ExcludedCard from "../members/ExcludedCard.vue";
import PendingList from "../members/PendingList.vue";
import ExcludedList from "../members/ExcludedList.vue";
import UnlistedList from "../members/UnlistedList.vue";
import { displayNames, excludedVariant, isMeExcluded, managedFacts, memberById, meId, partitionRows, excludedRowLine } from "../members/members-view.ts";
import { useMembersActions } from "../members/use-members-actions.ts";
import RejoinForm from "./RejoinForm.vue";
import NetTabHead from "./NetTabHead.vue";
import NetTabs from "./NetTabs.vue";
import PeerRow from "./PeerRow.vue";
import YouBlock from "./YouBlock.vue";
import AdminsLine from "./AdminsLine.vue";
import SecondAdminHint from "./SecondAdminHint.vue";
import DeletedBanner from "../members/DeletedBanner.vue";
import AdmissionRefused from "./AdmissionRefused.vue";
import { excludedByName } from "../members/excluded-by.ts";
import { useInviteNote } from "./invite-note.ts";
import { joiningTabs, netTabs, netWho } from "./net-tabs-view.ts";
import { PEERS_LOADING, PEERS_NETS_HINT_OK, PEERS_NO_STORE, peersNetsHint } from "./peers-texts.ts";
import { useNetsHint } from "./use-nets-hint.ts";
import AddNetCard from "../home/AddNetCard.vue";
import { leadsToArrival, settleExpected, watchArrivals, useNetBanner } from "../home/add-net-banner.ts";
import { arrivedBannerText } from "../home/add-net-actions.ts";
import { BANNER_OK, MAX_NETS } from "../home/add-net-texts.ts";
import { peersHash } from "../shell/nav.ts";
import { orderedPeers } from "./peers-view.ts";

const injected = inject(STORE_KEY);
if (!injected) throw new Error(PEERS_NO_STORE);
const store: Store = injected;
// The network on show: with several networks the chosen tab, otherwise the leading one.
const visible = computed(() => peersSlots(store.nets, store.slots));
const multi = computed(() => visible.value.length >= 2);
const joining = computed(() => joiningTabs(store));
// The row of tabs: the "multi" look (heads, hint) stays for several networks or a join in progress.
const crowded = computed(() => multi.value || joining.value.length > 0);
const showRow = computed(() => visible.value.length > 0 || joining.value.length > 0);
const addOnly = computed(() => showRow.value && store.peersTab === "add");
const slot = computed(() => (multi.value ? (store.peersTab && visible.value.includes(store.peersTab) ? store.peersTab : visible.value[0]) : store.lead));
const state = computed(() => stateOf(store, slot.value));
const members = computed(() => membersOf(store, slot.value));
const tabs = computed(() => netTabs(store));
const you = ref<InstanceType<typeof YouBlock> | null>(null);
const hint = useNetsHint(() => visible.value.length);
// The "added a network" banner sits on that network's tab; a join that waited for a member shows it on arrival.
const { banner, show: showBanner, clear: clearBanner } = useNetBanner();
// A form that waited for the network leads to its tab; a user who has gone elsewhere stays there.
// A join that the members refused leads from the form to that network's card, which says so and offers "Back".
watch(
  () => store.nets.map((n) => n.id).filter((id) => isRefused(store.nets, store.slots, id)),
  (now, before) => {
    const fresh = now.find((id) => !before.includes(id));
    if (!fresh) return;
    if (leadsToArrival(fresh, location.hash, store.peersTab)) location.hash = peersHash(fresh);
    settleExpected(fresh);
  },
);
watchArrivals(() => store.nets, (arrived) => {
  showBanner(arrived, arrivedBannerText(store, arrived));
  if (leadsToArrival(arrived, location.hash, store.peersTab)) location.hash = peersHash(arrived);
});
const waitingHere = computed(() => {
  const b = store.nets.find((n) => n.id === slot.value);
  return !!b && b.joining && b.id_hex == null && slot.value !== "main";
});
// A join that the members refused: the slot has no network yet, the node says why.
const refusedHere = computed(() => (slot.value === "main" || state.value?.network ? null : refusedText(state.value?.node?.invite_join)));
const { status, dialog, pick } = useMembersActions(store, () => slot.value);
const excluded = computed(() => isMeExcluded(state.value, members.value));
const variant = computed(() => excludedVariant(members.value, meId(state.value)));
const net = computed(() => netName(store.nets, slot.value));
// The way back for an excluded or removed computer is a form under the card, in this tab: the card's button opens it.
const rejoinOpen = ref(false);
const rejoin = ref<InstanceType<typeof RejoinForm> | null>(null);
// A join the members refused is told under the card, also when the form is closed (the slot has no network of its own then).
const refusalGate = useRefusalGate(() => state.value?.node?.invite_join);
const rejoinRefused = computed(() => refusedText(refusalGate.info.value));
async function openRejoin(): Promise<void> {
  rejoinOpen.value = true;
  await nextTick();
  rejoin.value?.focus();
}
const me = computed(() => meId(state.value));
const names = computed(() => displayNames(members.value));
const parts = computed(() => partitionRows(orderedPeers(state.value && state.value.peers), members.value));
// A peer the member list does not know is shown as a plain row: no menu, no badge.
const rows = computed(() => parts.value.active as PeerInfo[]);
const excludedLines = computed(() => parts.value.excluded.map((m) => excludedRowLine(m, names.value)));
const chosen = computed(() => chosenOf(store, slot.value, chosenExitIds(store)));
const adminIds = computed(() => menuCtx.value.adminIds);
const inviteNote = useInviteNote(store, () => slot.value);
const excludedBy = computed(() => excludedByName(state.value?.network?.excluded_by));
const menuCtx = computed(() => managedFacts(state.value?.network, members.value, me.value));
const memberOf = (p: PeerInfo) => memberById(members.value, p.id_full);

// Another tab: the line under the list is cleared
// unless the old tab is gone (a left network keeps its farewell line).
watch(slot, (now, before) => {
  rejoinOpen.value = false;
  if (before !== now && visible.value.includes(before)) status.value = { text: "", cls: "" };
});

// "Rename" with several networks leads to the name field of this tab; with one network it is the Home tab.
function onPick(key: string, idFull: string): void {
  if (key === "rename" && multi.value && you.value) you.value.focusName();
  else pick(key, idFull);
}

// The "Use as exit" button makes this member the only exit.
async function exitVia(idFull: string): Promise<void> {
  status.value = await saveExitIn(store, slot.value, idFull);
}
</script>

<template>
  <p v-if="!state && !addOnly" class="hint">{{ PEERS_LOADING }}</p>
  <template v-else>
    <p v-if="multi && !hint.seen.value" class="hint nets-hint" id="peers-nets-hint">{{ peersNetsHint(visible.length) }} <button type="button" class="btn btn-sm" id="peers-nets-hint-ok" @click="hint.dismiss">{{ PEERS_NETS_HINT_OK }}</button></p>
    <NetTabs v-if="showRow" :tabs="tabs" :active="addOnly ? 'add' : slot" :joining="joining" :full="visible.length >= MAX_NETS" />
    <AddNetCard v-if="addOnly" />
    <template v-else-if="excluded">
      <ExcludedCard members :variant="variant" :net="net" :by="excludedBy" :form-open="rejoinOpen" @join="openRejoin" />
      <RejoinForm v-if="rejoinOpen" ref="rejoin" :key="slot" :slot="slot" :net="net" :variant="variant" :refused="rejoinRefused" @attempt="refusalGate.dismiss" />
      <p v-else-if="rejoinRefused" class="field-error-text" id="rejoin-refused" role="alert"><strong>{{ REFUSED_LEAD(quotedNet(net)) }}</strong> {{ rejoinRefused }}</p>
    </template>
    <JoiningCard v-else-if="waitingHere || refusedHere" :key="slot" :slot="slot" :refused="refusedHere ?? undefined" :lead="refusedLead(state?.node?.invite_join)" />
    <template v-else-if="!state || !state.network">
      <NoNetwork />
      <p class="field-status" :class="status.cls" id="peers-exit-status" role="status" :hidden="!status.text">{{ status.text }}</p>
    </template>
    <template v-else>
      <NetTabHead :key="slot" :slot="slot" :name="netName(store.nets, slot)" :who="netWho(store, slot)" 
:multi="crowded" :note="inviteNote" />
      <AdminsLine v-if="adminIds.length > 0" :admins="adminIds" :me="me" :names="names" />
      <SecondAdminHint :only-admin="!!menuCtx.onlyAdmin" />
      <DeletedBanner />
      <AdmissionRefused :refused="refusedFlag(store, slot)" :members="members" :net="netName(store.nets, slot)" />
      <p v-if="banner && banner.slot === slot" class="hint net-banner" id="peers-net-banner" role="status">{{ banner.text }} <button type="button" class="btn btn-sm" id="peers-net-banner-ok" @click="clearBanner">{{ BANNER_OK }}</button></p>
      <div class="card">
        <ul class="peer-list" id="peers-list" role="list">
          <PeerRow v-for="p in rows" :key="p.id_full || p.id" :peer="p" :state="state!" :chosen="chosen" :member="memberOf(p)" :list="members" :me="me" :multi="multi" @exit-via="exitVia" @member-pick="onPick" />
        </ul>
        <p class="field-status" :class="status.cls" id="peers-exit-status" role="status" :hidden="!status.text">{{ status.text }}</p>
      </div>
      <PendingList :rows="parts.pending" :me="me" @pick="pick" />
      <ExcludedList :lines="excludedLines" />
      <UnlistedList :rows="parts.unlisted" @pick="pick" />
      <YouBlock v-if="multi" ref="you" :key="slot" :slot="slot" @leave="pick('leave', me)" @delete="pick('delete-network', me)" />
    </template>
    <ConfirmDialog ref="dialog" />
  </template>
</template>
