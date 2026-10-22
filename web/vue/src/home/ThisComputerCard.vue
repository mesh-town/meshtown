<script setup lang="ts">
// "This computer": the node name (saved on Enter and when leaving the field) and the "Allow
// others" switch. `bare` — without a card and title (step 3 of the setup).
import { inject } from "vue";
import { STORE_KEY } from "../core/store.ts";
import NodeNameField from "./NodeNameField.vue";
import { HOME_COMPUTER_ALLOW, HOME_COMPUTER_TITLE } from "./home-card-texts.ts";
import { inAppPage } from "./browser-setup.ts";
import { useHomeContext } from "./use-home.ts";

defineProps<{ bare?: boolean }>();
const inApp = inAppPage();
const { form, exitServer, ...h } = useHomeContext(inject(STORE_KEY)!);

async function onToggle(ev: Event): Promise<void> {
  const box = ev.target as HTMLInputElement;
  const res = await h.saveExitServer(box.checked);
  box.checked = res === "saved" ? box.checked : exitServer.value.checked;
}
</script>

<template>
  <div :class="{ card: !bare }" :id="bare ? undefined : 'home-computer'" :role="bare ? undefined : 'region'" :aria-labelledby="bare ? undefined : 'home-computer-title'">
    <h2 v-if="!bare" id="home-computer-title">{{ HOME_COMPUTER_TITLE }}</h2>
    <div id="home-name">
      <NodeNameField :with-save="inApp" />
    </div>
    <div v-if="!inApp" id="home-exit-server">
      <label class="toggle-inline home-exit-toggle">
        <span class="toggle-inline-text">{{ HOME_COMPUTER_ALLOW }}</span>
        <span class="toggle"><input type="checkbox" id="field-exit-enabled" :checked="exitServer.checked" :disabled="exitServer.readOnly" @change="onToggle"><span class="toggle-track"></span></span>
      </label>
      <p class="hint">{{ exitServer.hint }}</p>
      <p class="setting-desc cli-note" id="field-exit-enabled-cli-note" :hidden="!exitServer.cliNote">{{ exitServer.cliNote }}</p>
      <p class="field-status" :class="form.exitServerStatus.cls" id="field-exit-enabled-status" role="status" :hidden="!form.exitServerStatus.text">{{ form.exitServerStatus.text }}</p>
    </div>
  </div>
</template>
