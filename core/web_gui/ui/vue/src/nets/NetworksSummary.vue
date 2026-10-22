<script setup lang="ts">
// Step 2 of the setup on a router: "The proxy is open for…" with an "Edit" button, or the "router only" mode.
import { computed, inject, ref } from "vue";
import { STORE_KEY } from "../core/store.ts";
import { BTN_EDIT, BTN_OPEN_HOME } from "./nets-labels.ts";
import { NETS_NO_STORE, NETS_SUM_OPEN_FOR, ROUTER_ONLY_TEXT } from "./nets-texts.ts";
import { summaryView } from "./nets-view.ts";
import NetsSaved from "./NetsSaved.vue";
import NetworksBlock from "./NetworksBlock.vue";
import { useNets } from "./use-nets.ts";

const store = inject(STORE_KEY);
if (!store) throw new Error(NETS_NO_STORE);
const n = useNets(store);
const expanded = ref(false);
const running = computed(() => store.state?.socks?.access);
const wanted = computed(() => store.config?.socks?.access);
const sum = computed(() => summaryView(store.state ?? {}, store.config?.socks?.ifaces ?? []));
const collapse = (): void => { expanded.value = false; };
const openHome = (): Promise<boolean> => n.writeFields({ "socks.access": "localnet" });
</script>

<template>
  <div id="nets-wizard">
    <template v-if="running === 'localnet'">
      <div id="nets-summary" class="nets-sum">
        <span>{{ NETS_SUM_OPEN_FOR }} <template v-for="(t, i) in sum.open" :key="t"><template v-if="i">, </template><b>{{ t }}</b></template>.{{ sum.tail }}</span>
        <button type="button" id="nets-edit-btn" class="nets-link" :aria-expanded="expanded" @click="expanded = !expanded">{{ BTN_EDIT }}</button>
      </div>
      <NetworksBlock v-if="expanded" context="wizard" hide-saved @saved="collapse" />
    </template>
    <div v-else-if="wanted !== 'localnet'" id="router-only" class="nets-sum">
      <span>{{ ROUTER_ONLY_TEXT }}</span>
      <button type="button" id="nets-open-btn" class="btn btn-sm" :disabled="n.busy.value" @click="openHome">{{ BTN_OPEN_HOME }}</button>
      <span v-if="n.error.value" class="field-error-text">{{ n.error.value }}</span>
    </div>
    <NetsSaved v-if="n.showSaved.value" :phase="n.phase.value" :node-is-router="n.nodeIsRouter.value" show-restart @restart="n.restart" />
  </div>
</template>
