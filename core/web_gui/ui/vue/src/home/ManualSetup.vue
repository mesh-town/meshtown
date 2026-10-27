<script setup lang="ts">
// The manual block (SOCKS5) under a disclosure; `wizard` — the browser comes from the wizard, so no browser switcher.
import { inject } from "vue";
import type { StepValue } from "./browser-setup.ts";
import { MANUAL_TOGGLE, MANUAL_TOGGLE_OPEN } from "../common/pac-texts.ts";
import { STORE_KEY } from "../core/store.ts";
import { HOME_BROWSER_GROUP_LABEL, HOME_COPIED, HOME_COPY_ADDR, HOME_COPY_PORT, HOME_COPY_SHORT } from "./home-card-texts.ts";
import { browserList, copyAriaLabel, manualAvailable, manualBrowserHint, manualIntro } from "./pac-setup.ts";
import { useHomeContext } from "./use-home.ts";
import TermText from "./wizard/TermText.vue";
import { MANUAL_TOGGLE_HAND, MANUAL_TOGGLE_HAND_OPEN } from "./wizard/wizard-texts.ts";

defineProps<{ wizard?: boolean }>();
const COPY: Record<StepValue, [string, string]> = { pac: ["pac-copy-btn", HOME_COPY_ADDR], host: ["socks-copy-btn", HOME_COPY_ADDR], port: ["socks-copy-port-btn", HOME_COPY_PORT], lan: ["socks-copy-lan-btn", HOME_COPY_SHORT] };
const { form, proxy, ...h } = useHomeContext(inject(STORE_KEY)!);
</script>

<template>
  <template v-if="proxy.on && manualAvailable(proxy.os)">
    <button type="button" class="link-more" id="manual-toggle" :aria-expanded="form.browser.open" @click="h.toggleBrowser">{{ wizard ? (form.browser.open ? MANUAL_TOGGLE_HAND_OPEN : MANUAL_TOGGLE_HAND) : (form.browser.open ? MANUAL_TOGGLE_OPEN : MANUAL_TOGGLE) }}</button>
    <div v-if="form.browser.open" id="proxy-setup">
      <p class="hint" id="proxy-manual-intro"><TermText :text="manualIntro(proxy.os)" /></p>
      <template v-if="!wizard">
        <p class="hint" id="proxy-browser-hint">{{ manualBrowserHint(proxy.os) }}</p>
        <div id="browser-seg" role="group" :aria-label="HOME_BROWSER_GROUP_LABEL">
          <button v-for="[id, name] in browserList(proxy.os)" :key="id" type="button" class="btn btn-sm" :id="'browser-' + id" :data-browser="id" :aria-pressed="proxy.browser === id" @click="h.pickBrowser(id)">{{ name }}</button>
        </div>
      </template>
      <ol id="proxy-steps">
        <li v-for="(s, i) in proxy.steps" :key="i">{{ s.text }}<template v-if="s.copyKind">{{ " " }}<code class="mono" :id="'socks-' + s.copyKind + '-text'">{{ proxy[s.copyKind] }}</code> <button type="button" class="btn btn-sm" :id="COPY[s.copyKind][0]" :aria-label="copyAriaLabel(s.copyKind, form.browser.copied)" @click="h.copy(s.copyKind)">{{ form.browser.copied === s.copyKind ? HOME_COPIED : COPY[s.copyKind][1] }}</button></template></li>
      </ol>
      <p class="hint" id="proxy-copy-note" :hidden="!form.browser.note || form.browser.noteKind === 'pac'">{{ form.browser.note }}</p>
      <p class="hint" id="proxy-warn" :hidden="!proxy.warn">{{ proxy.warn }}</p>
    </div>
  </template>
</template>
