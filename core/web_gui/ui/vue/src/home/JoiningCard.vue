<script setup lang="ts">
// A join that waits for a member to confirm: the waiting line, a hint that turns into "no answer yet" after a minute,
// and "Cancel", which leaves the half-joined network. The slot is dropped from the page before `cancelled` is emitted.
import { inject, onMounted, onUnmounted, ref } from "vue";
import { api } from "../core/api.ts";
import { dropSlot } from "../core/overview.ts";
import { STORE_KEY } from "../core/store.ts";
import { leaveNet } from "../members/members-api.ts";
import { ADD_NET_WAITING, ADD_NET_WAITING_SUB, JOIN_BACK_BTN, JOIN_CANCEL_BTN, JOIN_CANCEL_FAILED, JOIN_NO_ANSWER, JOIN_NO_ANSWER_MS } from "./add-net-texts.ts";

const props = defineProps<{ slot: string; refused?: string; lead?: string }>();
const emit = defineEmits<{ cancelled: [] }>();
const store = inject(STORE_KEY)!;
const silent = ref(false);
const busy = ref(false);
const failed = ref(false);
let timer: ReturnType<typeof setTimeout> | null = null;

onMounted(() => {
  timer = setTimeout(() => (silent.value = true), JOIN_NO_ANSWER_MS);
});
onUnmounted(() => timer && clearTimeout(timer));

async function cancel(): Promise<void> {
  busy.value = true;
  failed.value = false;
  try {
    const reply = await leaveNet(api, props.slot);
    dropSlot(store, props.slot, reply.nets);
    emit("cancelled");
  } catch {
    failed.value = true;
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <div class="joining" id="net-joining" role="status">
    <p v-if="refused" class="field-error-text" id="net-join-refused"><strong v-if="lead">{{ lead }}</strong>{{ lead ? " " : "" }}{{ refused }}</p>
    <template v-else>
      <p class="hint">{{ ADD_NET_WAITING }}</p>
      <p class="hint">{{ silent ? JOIN_NO_ANSWER : ADD_NET_WAITING_SUB }}</p>
    </template>
    <div class="btn-row">
      <button type="button" class="btn" id="net-join-cancel" :disabled="busy" @click="cancel">{{ refused ? JOIN_BACK_BTN : JOIN_CANCEL_BTN }}</button>
    </div>
    <p v-if="failed" class="field-error-text" id="net-join-error" role="alert">{{ JOIN_CANCEL_FAILED }}</p>
  </div>
</template>
