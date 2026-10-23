<script setup lang="ts">
// "Exit via…" (like the tray submenu): a single choice of exit (`v-model` on `form.exitChoice`, not `:checked`
// from the store: on a PUT refusal the value returns, and Vue redraws the radio button) and "Check site".
// `bare` — without the card, the header and the site check (setup step 3); the `#exit-explain` paragraph about "exit"
// is always visible: it is the only place where the word is explained.
import { inject } from "vue";
import { exitViaText } from "../common/words.ts";
import { STORE_KEY } from "../core/store.ts";
import { countryTag, exitMetaText } from "./exit-list.ts";
import {
  HOME_EXIT_DIRECT, HOME_EXIT_EXPLAIN, HOME_EXIT_LIST_HINT, HOME_EXIT_NONE, HOME_EXIT_SEARCHING, HOME_EXIT_TITLE, HOME_FIND_ARIA, HOME_FIND_BTN, HOME_FIND_LEAD,
  HOME_FIND_NO_NET, HOME_FIND_PLACEHOLDER, findCheckingText, findEmptyText, findOkText,
} from "./home-card-texts.ts";
import { NETS_EXIT_ALL } from "./nets-texts.ts";
import { useHomeContext } from "./use-home.ts";

defineProps<{ bare?: boolean }>();
const { form, candidates, exitGroups, multi, findView, hasNet, searching, warning, ...h } = useHomeContext(inject(STORE_KEY)!);
</script>

<template>
  <div :class="{ card: !bare }" id="home-exit" :role="bare ? undefined : 'region'" :aria-labelledby="bare ? undefined : 'home-exit-title'">
    <h2 v-if="!bare" id="home-exit-title">{{ HOME_EXIT_TITLE }} <span v-if="multi" class="hint exit-all-nets">{{ NETS_EXIT_ALL }}</span></h2>
    <p class="hint" id="exit-explain">{{ HOME_EXIT_EXPLAIN }}</p>
    <p v-if="warning" id="exit-warning" class="alert">{{ warning.hint }}</p>
    <div id="exit-list">
      <label class="exit-row"><input type="radio" name="exit-choice" id="exit-direct" value="" v-model="form.exitChoice" @change="h.chooseDirect"><span class="exit-label-text">{{ HOME_EXIT_DIRECT }}</span></label>
      <template v-for="g in exitGroups" :key="g.slot">
        <h3 v-if="multi" class="exit-group-title" :data-slot="g.slot">{{ g.title }}</h3>
        <label v-for="c in g.items" :key="c.idFull" class="exit-row"><input type="radio" name="exit-choice" class="exit-choice" :data-id="c.idFull" :value="c.idFull" :disabled="c.blocked" v-model="form.exitChoice" @change="h.chooseExit(c.idFull)"><span class="exit-label-text" :data-id="c.idFull">{{ c.label }}{{ countryTag(c) }}</span><span class="exit-meta" :data-id="c.idFull">{{ exitMetaText(c) }}</span></label>
      </template>
      <p v-if="!candidates.length && searching" class="hint" id="exit-searching"><span class="spinner" aria-hidden="true"></span> {{ HOME_EXIT_SEARCHING }}</p>
      <p v-else-if="!candidates.length" class="hint">{{ HOME_EXIT_NONE }}</p>
    </div>
    <p v-if="!multi" class="hint" id="exit-list-hint" :hidden="!candidates.length">{{ HOME_EXIT_LIST_HINT }}</p>
    <p class="field-status" :class="form.exitStatus.cls" id="exit-list-status" role="status" :hidden="!form.exitStatus.text">{{ form.exitStatus.text }}</p>
    <template v-if="!bare">
      <div v-if="hasNet" class="hero-row">
        <span class="field-label-inline">{{ HOME_FIND_LEAD }}</span>
        <input type="text" class="input" id="find-exit-input" v-model="form.findDraft" :placeholder="HOME_FIND_PLACEHOLDER" :aria-label="HOME_FIND_ARIA">
        <button type="button" class="btn" id="find-exit-btn" @click="h.findExit">{{ HOME_FIND_BTN }}</button>
      </div>
      <p v-else class="hint" id="find-exit-no-network">{{ HOME_FIND_NO_NET }}</p>
      <p class="hint" id="find-exit-result" :hidden="findView.kind === 'none' && !form.findError">
        <template v-if="form.findError">{{ form.findError }}</template>
        <template v-else-if="findView.kind === 'checking'">{{ findCheckingText(findView.domain) }}</template>
        <template v-else-if="findView.kind === 'empty'">{{ findEmptyText(findView.domain) }}</template>
        <template v-else-if="findView.kind === 'ok'"><span class="find-exit-domain">{{ findOkText(findView.domain) }}</span> <button v-for="item in findView.items" :key="item.idFull" type="button" class="btn btn-sm" :data-exit-via="item.idFull" @click="h.chooseExit(item.idFull)">{{ exitViaText(item.label) }}</button></template>
      </p>
    </template>
  </div>
</template>
