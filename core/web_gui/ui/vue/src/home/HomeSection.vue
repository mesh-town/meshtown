<script setup lang="ts">
// The "Home" section: without a snapshot — "Loading…"; otherwise two tabs: "Main" (the status panel and the cards
// in `cardOrder` order) and "Setup" ("Three steps"). The shared `useHome` is passed to the cards through `provide`.
import { computed, inject, provide } from "vue";
import { STORE_KEY } from "../core/store.ts";
import BrowserCard from "./BrowserCard.vue";
import { inAppPage } from "./browser-setup.ts";
import HomeTabs from "./HomeTabs.vue";
import ExitCard from "./ExitCard.vue";
import NetsCard from "./NetsCard.vue";
import InviteCard from "./InviteCard.vue";
import JoinCard from "./JoinCard.vue";
import ExcludedCard from "../members/ExcludedCard.vue";
import { useExcluded } from "../members/use-excluded.ts";
import { isMeRemoved } from "../members/members-view.ts";
import { useMembersActions } from "../members/use-members-actions.ts";
import SetupWizard from "./SetupWizard.vue";
import StatePanel from "./StatePanel.vue";
import UpdateBanner from "./UpdateBanner.vue";
import ThisComputerCard from "./ThisComputerCard.vue";
import { HOME_LOADING } from "./home-card-texts.ts";
import { peersHash } from "../shell/nav.ts";
import { HOME_KEY, useHome } from "./use-home.ts";

const store = inject(STORE_KEY)!;
const inApp = inAppPage();
const home = useHome(store);
provide(HOME_KEY, home);
const { activeTab, panel, ring, order, multi, chips } = home;
// A computer that is out of the network: the card and the way back (an invite) instead of the usual cards.
const { excluded, variant, net } = useExcluded(store);
const removed = computed(() => isMeRemoved(store.members));
const outOfNet = computed(() => excluded.value || removed.value);
const { joinAnother } = useMembersActions(store);
</script>

<template>
  <div id="home-root">
    <div v-if="store.state === null" class="card"><p class="hint">{{ HOME_LOADING }}</p></div>
    <template v-else-if="outOfNet">
      <ExcludedCard :variant="variant" :net="net" @join="joinAnother" />
      <div class="card"><JoinCard :removed="removed && !excluded" /></div>
    </template>
    <template v-else>
      <HomeTabs />
      <div id="home-tabpanel" role="tabpanel" :aria-labelledby="'home-tab-' + activeTab">
        <SetupWizard v-if="activeTab === 'wizard'" />
        <template v-else>
          <UpdateBanner />
          <StatePanel :panel="panel" :ring="ring" @pick-other="home.pickOther" @direct="home.chooseDirect" />
          <div v-if="chips.length" class="net-chips" id="home-chips">
            <a v-for="c in chips" :key="c.slot" class="chip net-chip" :href="peersHash(c.slot)" :title="c.short">{{ c.text }}</a>
          </div>
          <div class="home-grid">
            <template v-for="key in order" :key="key">
              <NetsCard v-if="key === 'nets'" />
              <InviteCard v-else-if="key === 'invite'" full />
              <ExitCard v-else-if="key === 'exit'" class="card-full" />
              <div v-else class="mini-row">
                <BrowserCard v-if="!inApp" />
                <InviteCard v-if="!multi && !order.includes('invite')" />
                <ThisComputerCard v-if="!multi" />
              </div>
            </template>
          </div>
        </template>
      </div>
    </template>
  </div>
</template>
