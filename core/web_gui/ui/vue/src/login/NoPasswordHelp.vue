<script setup lang="ts">
// Step-by-step instructions when the page password is not set yet (instead of the sign-in form).
import { writeClipboard } from "../common/clipboard.ts";
import { NOPW_COPY } from "./login-texts.ts";
import { CMD_RESET, CMD_ROUTER, INTRO, ROUTER_NOTE, STEP_LINK, STEP_SET, showRouterLine, stepRunText } from "./no-password-help.ts";

const props = defineProps<{ host: string }>();

// The clipboard is unavailable or refused — the button stays silent, the command text is selected.
async function copy(text: string, ev: Event): Promise<void> {
  const ok = await writeClipboard({ clipboard: navigator.clipboard }, text);
  if (ok) return;
  const code = (ev.target as HTMLElement).parentElement?.querySelector("code");
  if (code) document.getSelection()?.selectAllChildren(code);
}
</script>

<template>
  <div id="no-password-help">
    <p class="hint">{{ INTRO }}</p>
    <ol class="no-password-steps">
      <li>
        {{ stepRunText(props.host) }}
        <div class="no-password-cmd">
          <code class="mono">{{ CMD_RESET }}</code>
          <button type="button" class="btn" @click="copy(CMD_RESET, $event)">{{ NOPW_COPY }}</button>
        </div>
        <template v-if="showRouterLine(props.host)">
          <p class="hint">{{ ROUTER_NOTE }}</p>
          <div class="no-password-cmd">
            <code class="mono">{{ CMD_ROUTER }}</code>
            <button type="button" class="btn" @click="copy(CMD_ROUTER, $event)">{{ NOPW_COPY }}</button>
          </div>
        </template>
      </li>
      <li>{{ STEP_LINK }}</li>
      <li>{{ STEP_SET }}</li>
    </ol>
  </div>
</template>
