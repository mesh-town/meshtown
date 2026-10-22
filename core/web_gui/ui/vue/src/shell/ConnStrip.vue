<script setup lang="ts">
// The connection strip: `strip` — `null | { kind, text }` (`kind: "changed"` — the settings were changed in another tab).
import { computed, ref } from "vue";
import type { Strip, StripViewKind } from "./conn.ts";
import { SHELL_RELOAD, SHELL_TOKEN_URL_APPLY, SHELL_TOKEN_URL_PLACEHOLDER } from "./shell-texts.ts";

// `hasSession` — the page has a session: the "token needed" strip is not shown (sign-in is by the sign-in screen).
const props = withDefaults(defineProps<{ strip?: Strip<StripViewKind> | null; hasSession?: boolean }>(), { strip: null, hasSession: false });
const shown = computed<Strip<StripViewKind> | null>(() => (props.hasSession && props.strip?.kind === "bad_token" ? null : props.strip));
const emit = defineEmits<{
  (e: "apply-token", text: string): void;
  (e: "reload"): void;
}>();
const pasted = ref<string>("");

// A link with a token does not stay in the field: after "Open" the field is empty.
function apply() {
  if (!pasted.value) return;
  emit("apply-token", pasted.value);
  pasted.value = "";
}
</script>

<template>
  <div id="conn-strip" class="warn-strip" role="alert" :hidden="!shown">
    <template v-if="shown && shown.kind === 'changed'">
      {{ shown.text }}
      <button id="conn-strip-reload" type="button" class="btn btn-sm" @click="$emit('reload')">{{ SHELL_RELOAD }}</button>
    </template>
    <template v-else-if="shown && shown.kind === 'bad_token'">
      {{ shown.text }}
      <input id="token-url-input" v-model="pasted" type="text" class="input input-token" autocomplete="off" spellcheck="false" :placeholder="SHELL_TOKEN_URL_PLACEHOLDER">
      <button id="token-url-apply" type="button" class="btn btn-sm" @click="apply">{{ SHELL_TOKEN_URL_APPLY }}</button>
    </template>
    <template v-else-if="shown">{{ shown.text }}</template>
  </div>
</template>
