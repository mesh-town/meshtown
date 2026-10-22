<script setup lang="ts">
// Password row in "Settings": "Set"/"Change" (the old password is not asked) and "Sign out".
// The node status is read on mount and after every settings save (`revision`): it shows
// that an access change has not taken effect yet (`access` ≠ `listening`).
import { computed, onMounted, ref, watch } from "vue";
import { errReason } from "../common/words.ts";
import { TOKEN_KEY } from "../core/token.ts";
import { authStatus, logout, setPassword } from "./login-api.ts";
import type { AuthStatus } from "./login-api.ts";
import { LOGIN_SET_PASSWORD, PASSWORD_CHANGE, PASSWORD_IS_SET, PASSWORD_LOGOUT, PASSWORD_NEEDS_RESTART, PASSWORD_NEW_LABEL, PASSWORD_NOT_SET, PASSWORD_ROW_TITLE, PASSWORD_SAVED, passwordLogoutFailedText, passwordSaveFailedText } from "./login-texts.ts";

const props = withDefaults(defineProps<{ revision?: number }>(), { revision: 0 });
const emit = defineEmits<{ (e: "loggedout"): void }>();

const status = ref<AuthStatus | null>(null);
const password = ref<string>("");
const message = ref<string>("");
const failed = ref<boolean>(false);
const busy = ref<boolean>(false);

const passwordSet = computed(() => !!status.value && status.value.password_set);
const needsRestart = computed(() => !!status.value && status.value.access !== status.value.listening);

async function refresh(): Promise<void> {
  status.value = await authStatus().catch(() => null);
}
onMounted(refresh);
watch(() => props.revision, refresh);

function report(text: string, isError: boolean): void {
  message.value = text;
  failed.value = isError;
}

async function save(): Promise<void> {
  if (busy.value || password.value === "") return;
  busy.value = true;
  const err = await setPassword(password.value).then(() => null, (e: unknown) => e as Error);
  busy.value = false;
  if (err) return report(passwordSaveFailedText(errReason(err)), true);
  password.value = "";
  report(PASSWORD_SAVED, false);
  await refresh();
}

async function leave(): Promise<void> {
  const err = await logout().then(() => null, (e: unknown) => e as Error);
  if (err) return report(passwordLogoutFailedText(errReason(err)), true);
  try {
    sessionStorage.removeItem(TOKEN_KEY);
  } catch (e) {
    // storage is unavailable — there was no token in it anyway
  }
  emit("loggedout");
}
</script>

<template>
  <div class="setting-row" id="password-row">
    <div class="setting-text">
      <div class="setting-title">{{ PASSWORD_ROW_TITLE }}</div>
      <div class="setting-desc">{{ passwordSet ? PASSWORD_IS_SET : PASSWORD_NOT_SET }}</div>
      <div v-if="needsRestart" id="access-restart-hint" class="setting-desc">{{ PASSWORD_NEEDS_RESTART }}</div>
      <div id="password-message" role="status" :class="failed ? 'field-error-text' : 'setting-desc'">{{ message }}</div>
    </div>
    <div class="setting-control">
      <input id="password-new" v-model="password" type="password" class="input" autocomplete="new-password" spellcheck="false" :aria-label="PASSWORD_NEW_LABEL" @keydown.enter="save">
      <button type="button" class="btn btn-sm" id="password-save-btn" :disabled="busy" @click="save">{{ passwordSet ? PASSWORD_CHANGE : LOGIN_SET_PASSWORD }}</button>
      <button type="button" class="btn btn-sm" id="logout-btn" @click="leave">{{ PASSWORD_LOGOUT }}</button>
    </div>
  </div>
</template>
