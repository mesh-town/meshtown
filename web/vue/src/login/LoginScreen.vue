<script setup lang="ts">
// Password sign-in to the page: `POST /api/auth/login`; success — the `done` event, errors — as a line under the field.
import { computed, ref } from "vue";
import { AuthError, login } from "./login-api.ts";
import { LOGIN_FAILED, LOGIN_HINT, LOGIN_NO_PASSWORD_TITLE, LOGIN_PASSWORD_LABEL, LOGIN_SUBMIT, LOGIN_TITLE, LOGIN_TOO_MANY_TRIES, LOGIN_WRONG_PASSWORD, loginTooManyTriesInText } from "./login-texts.ts";
import NoPasswordHelp from "./NoPasswordHelp.vue";

const props = defineProps<{ noPassword?: boolean }>();
const emit = defineEmits<{ (e: "done"): void }>();
const password = ref<string>("");
const error = ref<string>("");
const busy = ref<boolean>(false);
// No password: the node status said so in advance (prop) or the sign-in answered 409.
const missing = ref<boolean>(false);
const showHelp = computed(() => props.noPassword === true || missing.value);
const host = location.hostname;
const canSubmit = computed(() => password.value.length > 0 && !busy.value);

function wait(err: AuthError): string {
  if (err.retryAfterSec === null) return LOGIN_TOO_MANY_TRIES;
  return loginTooManyTriesInText(err.retryAfterSec);
}

const TEXTS: Record<number, (err: AuthError) => string> = {
  401: () => LOGIN_WRONG_PASSWORD,
  429: wait,
};

function errorText(err: unknown): string {
  const known = err instanceof AuthError ? TEXTS[err.status] : undefined;
  if (known) return known(err as AuthError);
  return err instanceof Error ? err.message : LOGIN_FAILED;
}

async function submit() {
  if (!canSubmit.value) return;
  busy.value = true;
  error.value = "";
  try {
    await login(password.value);
    emit("done");
  } catch (err) {
    missing.value = err instanceof AuthError && err.status === 409;
    error.value = errorText(err);
  } finally {
    password.value = "";
    busy.value = false;
  }
}
</script>

<template>
  <div class="auth-screen card">
    <template v-if="showHelp">
      <h2>{{ LOGIN_NO_PASSWORD_TITLE }}</h2>
      <NoPasswordHelp :host="host" />
    </template>
    <template v-else>
      <h2>{{ LOGIN_TITLE }}</h2>
      <p class="hint">{{ LOGIN_HINT }}</p>
      <form id="login-form" @submit.prevent="submit">
        <label class="field-label" for="login-password">{{ LOGIN_PASSWORD_LABEL }}</label>
        <input id="login-password" v-model="password" type="password" class="input auth-input" autocomplete="current-password" spellcheck="false">
        <p id="login-error" v-if="error" class="auth-error" role="alert">{{ error }}</p>
        <div class="btn-row">
          <button id="login-submit" type="submit" class="btn btn-primary" :disabled="!canSubmit">{{ LOGIN_SUBMIT }}</button>
        </div>
      </form>
    </template>
  </div>
</template>
