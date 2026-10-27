<script setup lang="ts">
// Setup step 2, under the browser card: what happens to sites while the node is not running. The answer is written at once.
import { computed, inject, ref, watch } from "vue";
import { STORE_KEY } from "../core/store.ts";
import { fieldErrorText, saveConfigFields } from "../core/save-config.ts";
import { HOME_WIZARD_STRICT_OFF, HOME_WIZARD_STRICT_ON, HOME_WIZARD_STRICT_Q } from "./home-card-texts.ts";
import { proxyView } from "./browser-setup.ts";

const store = inject(STORE_KEY)!;
const visible = computed(() => proxyView(store.state).on);
const fromState = computed(() => store.state?.socks?.strict === true);
// What the radios show: the node's answer, except for the moment of a write.
const shown = ref(fromState.value);
const error = ref("");
let inFlight = 0;
watch(fromState, (v) => { if (!inFlight) shown.value = v; });

async function choose(strict: boolean): Promise<void> {
  const changes = { "socks.strict": strict };
  shown.value = strict;
  error.value = "";
  inFlight++;
  const res = await saveConfigFields(store, changes);
  inFlight--;
  if (res.ok) return;
  error.value = fieldErrorText(res.error, changes);
  shown.value = fromState.value;
}
</script>

<template>
  <fieldset v-if="visible" id="strict-choice" class="strict-choice">
    <legend id="strict-q">{{ HOME_WIZARD_STRICT_Q }}</legend>
    <label class="exit-row"><input type="radio" name="strict-choice" id="strict-off" :checked="!shown" @change="choose(false)"><span class="exit-label-text">{{ HOME_WIZARD_STRICT_OFF }}</span></label>
    <label class="exit-row"><input type="radio" name="strict-choice" id="strict-on" :checked="shown" @change="choose(true)"><span class="exit-label-text">{{ HOME_WIZARD_STRICT_ON }}</span></label>
    <p v-if="error" id="strict-error" class="field-error-text" role="alert">{{ error }}</p>
  </fieldset>
</template>
