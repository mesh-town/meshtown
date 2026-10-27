<script setup lang="ts">
// "Connect the browser": the PAC address with copying, the system and steps, the manual block (SOCKS5) under a disclosure.
// `bare` — without the card, the header and the footer (setup step 2).
import { computed, inject, ref } from "vue";
import type { StepValue } from "./browser-setup.ts";
import {
  BROWSER_SCOPE_NOTE, FIREFOX_STEPS, MANUAL_TOGGLE, MANUAL_TOGGLE_OPEN, OS_LABEL, PAC_ADDR_LABEL, PAC_NO_EXIT_NOTE, PAC_TITLE, PAC_WHY, SCOPE_BROWSER,
  SCOPE_QUESTION, SCOPE_SYSTEM, WIZARD_AGAIN,
} from "../common/pac-texts.ts";
import { STORE_KEY } from "../core/store.ts";
import { isRouter } from "../lan/nets-view.ts";
import NetworksSummary from "../lan/NetworksSummary.vue";
import { PAC_OS_LIST, browserList, copyAriaLabel, manualAvailable, manualBrowserHint, manualIntro } from "./pac-setup.ts";
import {
  HOME_BROWSER_GROUP_LABEL, HOME_COPIED, HOME_COPY_ADDR, HOME_COPY_PORT, HOME_COPY_SHORT, HOME_LAN_LEAD_HOME, HOME_LAN_LEAD_MULTI,
  HOME_LAN_LEAD_SINGLE, HOME_LAN_TYPE, HOME_OS_GROUP_LABEL, HOME_PROXY_OFF, HOME_PROXY_OFF_LINK,
} from "./home-card-texts.ts";
import { readProxyScope, writeProxyScope } from "./proxy-scope.ts";
import type { ProxyScope } from "./proxy-scope.ts";
import { useHomeContext } from "./use-home.ts";

defineProps<{ bare?: boolean }>();
const COPY: Record<StepValue, [string, string]> = { pac: ["pac-copy-btn", HOME_COPY_ADDR], host: ["socks-copy-btn", HOME_COPY_ADDR], port: ["socks-copy-port-btn", HOME_COPY_PORT], lan: ["socks-copy-lan-btn", HOME_COPY_SHORT] };
const OS_IDS: Record<string, string> = { mac: "os-mac", windows: "os-windows", linux: "os-linux", phone: "os-phone" };
const store = inject(STORE_KEY)!;
const { form, proxy, ...h } = useHomeContext(store);
const router = computed(() => isRouter(store.state ?? {}));
const SCOPES: [ProxyScope, string][] = [["system", SCOPE_SYSTEM], ["browser", SCOPE_BROWSER]];
const storage = typeof localStorage === "undefined" ? null : localStorage;
const scope = ref<ProxyScope | null>(readProxyScope(storage));
const pickScope = (value: ProxyScope): void => {
  scope.value = value;
  writeProxyScope(value, storage);
};
const LAN_LEAD = { legacy: HOME_LAN_LEAD_SINGLE, single: HOME_LAN_LEAD_SINGLE, home: HOME_LAN_LEAD_HOME, multi: HOME_LAN_LEAD_MULTI };
</script>

<template>
  <div :class="{ card: !bare }" id="home-proxy" :role="bare ? undefined : 'region'" :aria-labelledby="bare ? undefined : 'home-proxy-title'">
    <h2 v-if="!bare" id="home-proxy-title">{{ PAC_TITLE }}</h2>
    <p id="proxy-hint">
      <template v-if="!proxy.on">{{ HOME_PROXY_OFF }} <a href="#settings">{{ HOME_PROXY_OFF_LINK }}</a></template>
      <template v-else>{{ proxy.intro }}</template>
    </p>
    <template v-if="proxy.on">
      <p id="pac-row">{{ PAC_ADDR_LABEL }} <code class="mono" id="pac-url-text">{{ proxy.pacUrl }}</code> <button type="button" class="btn btn-sm" :id="COPY.pac[0]" :aria-label="copyAriaLabel('pac', form.browser.copied)" @click="h.copy('pac')">{{ form.browser.copied === "pac" ? HOME_COPIED : COPY.pac[1] }}</button></p>
      <p class="hint" id="pac-copy-note" :hidden="form.browser.noteKind !== 'pac' || !form.browser.note">{{ form.browser.note }}</p>
      <p class="hint" id="pac-why">{{ PAC_WHY }}</p>
      <p v-if="!form.exitChoice && !router" class="hint" id="pac-no-exit">{{ PAC_NO_EXIT_NOTE }}</p>
      <NetworksSummary v-if="bare && router" />
      <p v-if="proxy.lanView">
        <span id="proxy-lan">{{ LAN_LEAD[proxy.lanView.kind] }} <template v-for="(a, i) in proxy.lanView.items" :key="a.addr"><template v-if="i"> · </template><template v-if="proxy.lanView.kind === 'multi'">{{ a.title }} — </template><code class="mono lan-addr">{{ a.addr }}</code><template v-if="proxy.lanView.kind === 'legacy' || proxy.lanView.kind === 'single'">{{ " " }}<small class="lan-iface">({{ a.title }})</small></template></template>{{ HOME_LAN_TYPE }}</span> <button type="button" class="btn btn-sm" :id="COPY.lan[0]" :aria-label="copyAriaLabel('lan', form.browser.copied)" @click="h.copy('lan')">{{ form.browser.copied === "lan" ? HOME_COPIED : COPY.lan[1] }}</button><template v-if="proxy.lanView.hint"><br><span class="hint" id="proxy-lan-hint">{{ proxy.lanView.hint }}</span></template>
      </p>
      <p v-if="!form.exitChoice && router" class="hint" id="pac-no-exit">{{ PAC_NO_EXIT_NOTE }}</p>
      <span id="scope-q">{{ SCOPE_QUESTION }}</span>
      <div id="scope-seg" role="group" aria-labelledby="scope-q">
        <button v-for="[id, name] in SCOPES" :key="id" type="button" class="btn btn-sm" :id="'scope-' + id" :aria-pressed="scope === id" @click="pickScope(id)">{{ name }}</button>
      </div>
      <template v-if="scope === 'system'">
        <span id="os-label">{{ OS_LABEL }}</span>
        <div id="os-seg" role="group" :aria-label="HOME_OS_GROUP_LABEL">
          <button v-for="[id, name] in PAC_OS_LIST" :key="id" type="button" class="btn btn-sm" :id="OS_IDS[id]" :data-os="id" :aria-pressed="proxy.os === id" @click="h.pickOs(id)">{{ name }}</button>
        </div>
        <ol v-if="proxy.pac.steps.length" id="pac-steps">
          <li v-for="(t, i) in proxy.pac.steps" :key="i">{{ t }}</li>
        </ol>
        <p class="hint" id="pac-os-note" :hidden="!proxy.pac.note">{{ proxy.pac.note }}</p>
        <p v-for="(t, i) in proxy.pac.small" :key="i" class="hint pac-small">{{ t }}</p>
      </template>
      <template v-else-if="scope === 'browser'">
        <ol id="firefox-steps">
          <li v-for="(t, i) in FIREFOX_STEPS" :key="i">{{ t }}</li>
        </ol>
        <p class="hint" id="browser-scope-note">{{ BROWSER_SCOPE_NOTE }}</p>
      </template>
      <button v-if="scope && manualAvailable(proxy.os)" type="button" class="link-more" id="manual-toggle" :aria-expanded="form.browser.open" @click="h.toggleBrowser">{{ form.browser.open ? MANUAL_TOGGLE_OPEN : MANUAL_TOGGLE }}</button>
    </template>
    <div v-if="proxy.on && scope && manualAvailable(proxy.os) && form.browser.open" id="proxy-setup">
      <p class="hint" id="proxy-manual-intro">{{ manualIntro(proxy.os) }}</p>
      <p class="hint" id="proxy-browser-hint">{{ manualBrowserHint(proxy.os) }}</p>
      <div id="browser-seg" role="group" :aria-label="HOME_BROWSER_GROUP_LABEL">
        <button v-for="[id, name] in browserList(proxy.os)" :key="id" type="button" class="btn btn-sm" :id="'browser-' + id" :data-browser="id" :aria-pressed="proxy.browser === id" @click="h.pickBrowser(id)">{{ name }}</button>
      </div>
      <ol id="proxy-steps">
        <li v-for="(s, i) in proxy.steps" :key="i">{{ s.text }}<template v-if="s.copyKind">{{ " " }}<code class="mono" :id="'socks-' + s.copyKind + '-text'">{{ proxy[s.copyKind] }}</code> <button type="button" class="btn btn-sm" :id="COPY[s.copyKind][0]" :aria-label="copyAriaLabel(s.copyKind, form.browser.copied)" @click="h.copy(s.copyKind)">{{ form.browser.copied === s.copyKind ? HOME_COPIED : COPY[s.copyKind][1] }}</button></template></li>
      </ol>
      <p class="hint" id="proxy-copy-note" :hidden="!form.browser.note || form.browser.noteKind === 'pac'">{{ form.browser.note }}</p>
      <p class="hint" id="proxy-warn" :hidden="!proxy.warn">{{ proxy.warn }}</p>
    </div>
    <p v-if="!bare" class="hint"><button type="button" class="link-more" @click="h.setMark('browser')">{{ WIZARD_AGAIN }}</button></p>
  </div>
</template>
