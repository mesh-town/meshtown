<script setup lang="ts">
import { NET_NAME } from "../common/words.ts";
import { DIAG_KEY_HIDE, DIAG_KEY_HINT_HEAD, DIAG_KEY_HINT_TAIL, DIAG_KEY_SHOW, DIAG_KEY_TITLE } from "./diag-texts.ts";
import type { Diagnostics } from "./use-diagnostics.ts";

const props = defineProps<{ diag: Diagnostics }>();
const { networkKeyState, keyText, toggleKey } = props.diag;
</script>

<template>
  <div class="card" id="net-key-card" role="region" aria-labelledby="net-key-title">
    <h2 id="net-key-title">{{ DIAG_KEY_TITLE }}</h2>
    <div class="key-row">
      <code class="key-mask mono" id="netkey">{{ keyText }}</code>
      <button type="button" class="btn btn-sm" id="netkey-show-btn" :disabled="networkKeyState.busy" @click="toggleKey">{{ networkKeyState.shown ? DIAG_KEY_HIDE : DIAG_KEY_SHOW }}</button>
    </div>
    <p class="hint">{{ DIAG_KEY_HINT_HEAD }}{{ NET_NAME }}{{ DIAG_KEY_HINT_TAIL }}</p>
    <p v-if="networkKeyState.error" class="hint" role="status">{{ networkKeyState.error }}</p>
  </div>
</template>
