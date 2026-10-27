<script setup lang="ts">
// The "Home" section: without a snapshot — "Loading…"; otherwise two tabs: "Main" (the status panel and the cards
// in `cardOrder` order) and "Setup" ("Three steps"). The shared `useHome` is passed to the cards through `provide`.
import { computed, inject, provide } from "vue";
import { STORE_KEY } from "../core/store.ts";
import { netName, refusedFlag } from "../core/slots.ts";
import BrowserCard from "./BrowserCard.vue";
import { inAppPage } from "./browser-setup.ts";
import HomeTabs from "./HomeTabs.vue";
import ExitCard from "./ExitCard.vue";
import NetsCard from "./NetsCard.vue";
import InviteCard from "./InviteCard.vue";
import JoinCard from "./JoinCard.vue";
import ExcludedCard from "../members/ExcludedCard.vue";
import { useExcluded } from "../members/use-excluded.ts";
import SetupWizard from "./SetupWizard.vue";
import StatePanel from "./StatePanel.vue";
import UpdateBanner from "./UpdateBanner.vue";
import AdmissionRefused from "../peers/AdmissionRefused.vue";
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
</script>

<template>
  <div id="home-root">
    <div v-if="store.state === null" class="card"><p class="hint">{{ HOME_LOADING }}</p></div>
    <template v-else-if="excluded">
      <ExcludedCard form-open :variant="variant" :net="net" />
      <div class="card"><JoinCard /></div>
    </template>
    <template v-else>
      <HomeTabs />
      <div id="home-tabpanel" role="tabpanel" :aria-labelledby="'home-tab-' + activeTab">
        <!-- One plate above the sub-tabs: the instance is not recreated when the sub-tab changes, so its 15-second count goes on. "active" draws it only in the Home section: the id is unique. -->
        <AdmissionRefused :active="store.activeSection === 'home'" :refused="refusedFlag(store, 'main')" :members="store.members" :net='netName(store.nets, "main")' />
        <template v-if="activeTab === 'wizard'">
          <SetupWizard />
        </template>
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
