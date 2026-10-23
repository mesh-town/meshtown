<script setup lang="ts">
// The page skeleton: the connection and identity strips, tabs, the header with the status line, seven sections.
// All sections are mounted at once, visibility is `hidden`; the "section → component" dictionary is here
// (`nav.ts` does not import `.vue`: it is loaded by the node project without the vue plugin).
import { computed, inject } from "vue";
import type { Component } from "vue";
import { STORE_KEY } from "../core/store.ts";
import type { Store } from "../core/store.ts";
import DomainsSection from "../domains/DomainsSection.vue";
import HomeSection from "../home/HomeSection.vue";
import LogSection from "../log/LogSection.vue";
import DiagnosticsSection from "../network/DiagnosticsSection.vue";
import PeersSection from "../peers/PeersSection.vue";
import PortsSection from "../ports/PortsSection.vue";
import LoginScreen from "../login/LoginScreen.vue";
import SetupScreen from "../login/SetupScreen.vue";
import { totalVotes, votesTarget } from "../peers/net-votes.ts";
import { useAuth } from "../login/use-auth.ts";
import SettingsSection from "../settings/SettingsSection.vue";
import ConnStrip from "./ConnStrip.vue";
import IdentityStrip from "./IdentityStrip.vue";
import TabsNav from "./TabsNav.vue";
import TopBar from "./TopBar.vue";
import { inAppPage } from "../home/browser-setup.ts";
import { sectionViews, viewTitle } from "./nav.ts";
import type { SectionId } from "./nav.ts";
import { useNarrow } from "../common/use-narrow.ts";
import { useFavicon } from "./use-favicon.ts";
import { SHELL_NO_STORE } from "./shell-texts.ts";
import { useShell } from "./use-shell.ts";

const views = sectionViews(inAppPage());
const narrow = useNarrow();
const VIEWS: Record<SectionId, Component> = {
  home: HomeSection,
  peers: PeersSection,
  domains: DomainsSection,
  ports: PortsSection,
  network: DiagnosticsSection,
  log: LogSection,
  settings: SettingsSection,
};

const injected = inject(STORE_KEY);
if (!injected) throw new Error(SHELL_NO_STORE);
const store: Store = injected;
const { conn, status, strip, identity, ready, applyToken, reloadConfig, resume } = useShell(store);
useFavicon(store);
// Removals and join requests waiting for this member's confirmation, summed over the networks.
const votes = computed(() => totalVotes(store));
const votesHref = computed(() => votesTarget(store));
const { screen, hasSession, noPassword, loggedIn, passwordSet } = useAuth(conn);

function onLoggedIn(): void {
  loggedIn();
  resume();
}
</script>

<template>
  <div class="shell" :data-ready="ready ? 'true' : null">
    <ConnStrip :strip="screen === 'app' ? strip : null" :has-session="hasSession" @apply-token="applyToken" @reload="reloadConfig" />
    <LoginScreen v-if="screen === 'login'" :no-password="noPassword" @done="onLoggedIn" />
    <SetupScreen v-else-if="screen === 'setup'" @done="passwordSet" />
    <IdentityStrip v-if="screen === 'app'" :identity="identity" />
    <div v-if="screen === 'app'" class="layout">
      <TabsNav :active="store.activeSection" :dirty="store.settingsDirty" :votes="votes" />
      <div class="content-col">
        <TopBar :status="status" :votes="votes" :href="votesHref" />
        <main class="page">
          <section
            v-for="v in views"
            :id="'sec-' + v.id"
            :key="v.id"
            class="tab-panel"
            role="tabpanel"
            :aria-labelledby="'tab-' + v.id"
            :hidden="store.activeSection !== v.id"
          >
            <h2 v-if="v.title" class="tab-title">{{ viewTitle(v, narrow) }}</h2>
            <component :is="VIEWS[v.id]" />
          </section>
        </main>
      </div>
    </div>
  </div>
</template>
