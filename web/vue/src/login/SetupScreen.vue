<script setup lang="ts">
// First sign-in with a temporary token: the page password twice and the access choice.
// Order: `PUT /api/auth/password`, then `PUT /api/config` with `web.access`.
import { computed, inject, ref } from "vue";
import { saveConfigFields } from "../core/save-config.ts";
import { STORE_KEY } from "../core/store.ts";
import type { Store } from "../core/store.ts";
import { errReason } from "../common/words.ts";
import { WEB_ACCESS_OPTIONS } from "../settings/settings-fields.ts";
import { setPassword } from "./login-api.ts";
import type { AuthAccess } from "./login-api.ts";
import { LOGIN_SET_PASSWORD, SETUP_ACCESS_HINT, SETUP_ACCESS_LABEL, SETUP_AGAIN_LABEL, SETUP_CONTINUE, SETUP_HINT, SETUP_MISMATCH, SETUP_NO_STORE, SETUP_TITLE, setupAccessFailedText, setupPasswordFailedText, setupPasswordLabelText } from "./login-texts.ts";

// "From this computer and the local network" goes first: it is selected by default.
const ACCESS_CHOICES = [...WEB_ACCESS_OPTIONS].reverse();
const MIN_LENGTH = 8;
const MAX_LENGTH = 256;

const emit = defineEmits<{ (e: "done"): void }>();
const injected = inject(STORE_KEY);
if (!injected) throw new Error(SETUP_NO_STORE);
const store: Store = injected;

const first = ref<string>("");
const second = ref<string>("");
const access = ref<AuthAccess>("localnet");
const error = ref<string>("");
const busy = ref<boolean>(false);
const passwordSaved = ref<boolean>(false);

const longEnough = computed(() => first.value.length >= MIN_LENGTH && first.value.length <= MAX_LENGTH);
const mismatch = computed(() => second.value.length > 0 && first.value !== second.value);
const canSubmit = computed(() => longEnough.value && first.value === second.value && !busy.value);

async function savePassword(): Promise<boolean> {
  if (passwordSaved.value) return true;
  const failed = await setPassword(first.value).then(() => false, (err: unknown) => {
    error.value = setupPasswordFailedText(errReason(err as Error));
    return true;
  });
  passwordSaved.value = !failed;
  return !failed;
}

async function saveAccess(): Promise<boolean> {
  const res = await saveConfigFields(store, { "web.access": access.value });
  if (!res.ok) error.value = setupAccessFailedText(errReason(res.error));
  return res.ok;
}

async function submit() {
  if (!canSubmit.value) return;
  busy.value = true;
  error.value = "";
  const done = (await savePassword()) && (await saveAccess());
  busy.value = false;
  if (done) emit("done");
}
</script>

<template>
  <div class="auth-screen card">
    <h2>{{ SETUP_TITLE }}</h2>
    <p class="hint">{{ SETUP_HINT }}</p>
    <form id="setup-form" @submit.prevent="submit">
      <label class="field-label" for="setup-password">{{ setupPasswordLabelText(MIN_LENGTH) }}</label>
      <input id="setup-password" v-model="first" type="password" class="input auth-input" autocomplete="new-password" spellcheck="false">
      <label class="field-label" for="setup-password2">{{ SETUP_AGAIN_LABEL }}</label>
      <input id="setup-password2" v-model="second" type="password" class="input auth-input" autocomplete="new-password" spellcheck="false">
      <p v-if="mismatch" class="auth-error">{{ SETUP_MISMATCH }}</p>
      <label class="field-label" for="setup-access">{{ SETUP_ACCESS_LABEL }}</label>
      <select id="setup-access" v-model="access" class="input auth-input">
        <option v-for="o in ACCESS_CHOICES" :key="o[0]" :value="o[0]">{{ o[1] }}</option>
      </select>
      <p class="hint">{{ SETUP_ACCESS_HINT }}</p>
      <p id="setup-error" v-if="error" class="auth-error" role="alert">{{ error }}</p>
      <div class="btn-row">
        <button id="setup-submit" type="submit" class="btn btn-primary" :disabled="!canSubmit">{{ passwordSaved ? SETUP_CONTINUE : LOGIN_SET_PASSWORD }}</button>
      </div>
    </form>
  </div>
</template>
