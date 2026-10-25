<script setup lang="ts">
import { computed, inject, nextTick, ref } from "vue";
import { STOPPED_NOTE, STOPPED_NOTE_APP } from "../common/pac-texts.ts";
import { NET_NAME } from "../common/words.ts";
import { inAppPage } from "../home/browser-setup.ts";
import { STORE_KEY } from "../core/store.ts";
import { isMulti, stateOf, visibleSlots } from "../core/slots.ts";
import DeletedBanner from "../members/DeletedBanner.vue";
import { canDeleteNet } from "../members/managed-ctx.ts";
import { MENU_DELETE_NETWORK } from "../members/members-managed-texts.ts";
import { targetSlots } from "../core/save-config.ts";
import { useExcluded } from "../members/use-excluded.ts";
import ConfirmDialog from "../members/ConfirmDialog.vue";
import { useMembersActions } from "../members/use-members-actions.ts";
import { meId } from "../members/members-ids.ts";
import NodeNameField from "../home/NodeNameField.vue";
import AddNetCard from "../home/AddNetCard.vue";
import SettingField from "./SettingField.vue";
import SettingsSaveBar from "./SettingsSaveBar.vue";
import PasswordRow from "../login/PasswordRow.vue";
import TokenRow from "./TokenRow.vue";
import type { SaveOutcome } from "./settings-actions.ts";
import { EXPERT_FIELDS, MAIN_FIELDS, PAGE_FIELDS, fieldErrorLine, socksAccessDesc } from "./settings-fields.ts";
import type { Field } from "./settings-fields.ts";
import { fieldCaption, fieldLabel } from "./settings-view.ts";
import NetworksBlock from "../lan/NetworksBlock.vue";
import { blockWhere, sameNetsLine } from "../lan/nets-view.ts";
import {
  SETTINGS_BAD_FILE_NAME, SETTINGS_BROKEN_HINT, SETTINGS_BROKEN_TITLE, SETTINGS_CONFLICT_HINT, SETTINGS_CONFLICT_OVERWRITE,
  SETTINGS_CONFLICT_RELOAD, SETTINGS_CONFLICT_TITLE, SETTINGS_EXPERT_TITLE, SETTINGS_FIELD_ERRORS_HINT, SETTINGS_FILE_LABEL,
  SETTINGS_LOADING, SETTINGS_MAIN_TITLE, SETTINGS_NET_TITLE, SETTINGS_PAGE_TITLE, SETTINGS_RESTART_NOTE, SETTINGS_SAME_NETS_LINK,
  SETTINGS_LEAVE_BTN, SETTINGS_LEAVE_DELETE_MULTI, SETTINGS_LEAVE_MULTI, SETTINGS_LEAVE_TITLE, SETTINGS_NAME_PER_NET, SETTINGS_STOP_ALL, SETTINGS_STOP_PREFIX, SETTINGS_STORE_MISSING, leaveHint,
} from "./settings-texts.ts";
import { useSettings } from "./use-settings.ts";

const store = inject(STORE_KEY);
if (!store) throw new Error(SETTINGS_STORE_MISSING);
const s = useSettings(store);
const { bar, banner } = s; // top level — refs are unwrapped in the template
const root = ref<HTMLElement | null>(null);
const { excluded } = useExcluded(store);
const { status: leaveStatus, dialog, pick } = useMembersActions(store);
const multi = computed(() => isMulti(store));
const anyManaged = computed(() => visibleSlots(store.nets, store.slots).some((id) => store.slots[id]?.brief.kind === "Managed" || stateOf(store, id)?.network?.kind === "Managed"));
const canDelete = computed(() => canDeleteNet(store.state?.network));
const hasKeyNet = computed(() => targetSlots(store).length > 0);
const labeled = (f: Field): Field => ({ ...f, title: fieldLabel(f, multi.value) });
const ready = computed(() => !!(store.config && store.configMeta));

// After a 422 rejection — scroll to the first field with an error.
function focusFirstError(container: HTMLElement | null): void {
  const el = container && container.querySelector<HTMLElement>(".field-error");
  if (!el) return;
  el.scrollIntoView({ behavior: "smooth", block: "center" });
  el.focus();
}
async function afterSave(status: SaveOutcome): Promise<void> {
  if (status !== "invalid") return;
  await nextTick();
  focusFirstError(root.value);
}
const where = computed(() => (store.config ? blockWhere(store.config, store.state ?? {}) : null));
const sameNets = computed(() => (store.config ? sameNetsLine(store.config, store.state ?? {}) : null));
const mainFields = computed(() => MAIN_FIELDS.map((f) => (f.path === "socks.access" ? { ...f, desc: socksAccessDesc(where.value === "main") } : f)));
const showSameNets = (): void => document.getElementById("nets-block")?.scrollIntoView();
const reloadPage = (): void => location.reload();
const save = async () => afterSave(await s.save());
const overwrite = async () => afterSave(await s.overwrite());
const onToggle = (ev: Event): void => { s.form.expertOpen = (ev.target as HTMLDetailsElement).open; };
</script>

<template>
  <div id="settings-root" ref="root">
    <p class="hint">{{ SETTINGS_RESTART_NOTE }}</p>
    <div id="settings-main">
      <div v-if="!ready" class="card"><p class="hint">{{ SETTINGS_LOADING }}</p></div>
      <template v-else>
        <div id="settings-conflict-slot">
          <div v-if="s.form.conflict" class="card card-danger">
            <h2>{{ SETTINGS_CONFLICT_TITLE }}</h2>
            <p class="hint">{{ SETTINGS_CONFLICT_HINT }}</p>
            <div class="btn-row">
              <button type="button" class="btn" id="settings-conflict-reload-btn" @click="s.reload">{{ SETTINGS_CONFLICT_RELOAD }}</button>
              <button type="button" class="btn btn-primary" id="settings-conflict-overwrite-btn" @click="overwrite">{{ SETTINGS_CONFLICT_OVERWRITE }}</button>
            </div>
          </div>
        </div>
        <div v-if="banner" id="settings-file-banner" role="region" :aria-label="SETTINGS_FILE_LABEL">
          <div v-if="banner.broken" class="card card-danger">
            <h2>{{ SETTINGS_BROKEN_TITLE }}</h2>
            <p class="hint">{{ banner.broken.message }}</p>
            <p class="hint">{{ SETTINGS_BROKEN_HINT }} <code class="mono">{{ SETTINGS_BAD_FILE_NAME }}</code>.</p>
          </div>
          <div v-if="banner.fieldErrors && banner.fieldErrors.length" class="card card-muted">
            <p class="hint">{{ SETTINGS_FIELD_ERRORS_HINT }}</p>
            <ul class="file-errors-list">
              <li v-for="fe in banner.fieldErrors" :key="fe.path">{{ fieldErrorLine(fe) }}</li>
            </ul>
          </div>
        </div>
        <div class="card" id="settings-main-card">
          <h2>{{ SETTINGS_MAIN_TITLE }}</h2>
          <div id="settings-node-name-row"><p v-if="multi" id="settings-name-per-net" class="hint">{{ SETTINGS_NAME_PER_NET }}</p><NodeNameField v-else id-base="settings-node-name" /></div>
          <template v-for="f in mainFields" :key="f.path">
            <SettingField :field="f" :view="s.view(f)" @edit="s.edit" />
            <NetworksBlock v-if="f.path === 'socks.access' && where === 'main'" context="settings" />
          </template>
        </div>
      </template>
    </div>

    <div class="card" id="settings-net-card">
      <h2>{{ SETTINGS_NET_TITLE }}</h2>
      <div id="net-join-slot"><AddNetCard v-if="hasKeyNet && !excluded" context="settings" /></div>
      <button type="button" class="btn btn-danger" id="stop-node-btn" @click="s.stop">{{ SETTINGS_STOP_PREFIX }}{{ NET_NAME }}</button>
      <p class="hint">{{ multi ? SETTINGS_STOP_ALL : "" }}{{ inAppPage() ? STOPPED_NOTE_APP : STOPPED_NOTE }}</p>
      <DeletedBanner />
      <p v-if="multi" id="settings-leave-multi" class="hint">{{ anyManaged ? SETTINGS_LEAVE_DELETE_MULTI : SETTINGS_LEAVE_MULTI }}</p>
      <div v-else-if="store.state && store.state.network && !excluded" id="settings-leave-row">
        <h3 class="setting-title">{{ SETTINGS_LEAVE_TITLE }}</h3>
        <p class="hint">{{ leaveHint(store.state.network.label) }}</p>
        <button type="button" class="btn btn-danger" id="settings-leave-btn" @click="pick('leave', meId(store.state))">{{ SETTINGS_LEAVE_BTN }}</button>
        <button v-if="canDelete" type="button" class="btn btn-danger" id="settings-delete-net-btn" @click="pick('delete-network', meId(store.state))">{{ MENU_DELETE_NETWORK }}</button>
      </div>
      <p id="settings-leave-status" role="status" class="field-status" :class="leaveStatus.cls">{{ leaveStatus.text }}</p>
      <ConfirmDialog ref="dialog" />
    </div>

    <template v-if="ready">
      <div id="settings-expert">
        <details class="card" id="settings-expert-details" :open="s.form.expertOpen" @toggle="onToggle">
          <summary class="setting-title">{{ SETTINGS_EXPERT_TITLE }}</summary>
          <SettingField v-for="f in EXPERT_FIELDS" :key="f.path" :field="labeled(f)" :caption="fieldCaption(f, multi)" :view="s.view(f)" @edit="s.edit" />
          <h3 class="setting-title">{{ SETTINGS_PAGE_TITLE }}</h3>
          <template v-for="f in PAGE_FIELDS" :key="f.path">
            <SettingField :field="labeled(f)" :caption="fieldCaption(f, multi)" :view="s.view(f)" @edit="s.edit" />
            <NetworksBlock v-if="f.path === 'web.access' && where === 'page'" context="settings" />
            <p v-if="f.path === 'web.access' && sameNets" id="same-nets" class="nets-same">{{ sameNets }} <button type="button" id="same-nets-link" class="nets-link" @click="showSameNets">{{ SETTINGS_SAME_NETS_LINK }}</button></p>
          </template>
          <PasswordRow :revision="store.configRev ?? 0" @loggedout="reloadPage" />
          <TokenRow :revealed="s.form.tokenRevealed" :token="s.form.token" @toggle="s.toggleToken" @rotate="s.rotate" />
        </details>
      </div>
      <SettingsSaveBar :bar="bar" :message-cls="s.form.message.cls" @save="save" @reset="s.reset" @restart="s.restart" />
    </template>
  </div>
</template>
