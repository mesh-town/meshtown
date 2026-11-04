<script setup lang="ts">
import { inject, ref, useId, watch } from "vue";
import { api } from "../core/api.ts";
import { slotPath, stateOf } from "../core/slots.ts";
import { STORE_KEY } from "../core/store.ts";
import { HOME_INVITE_FOR_LABEL } from "./home-card-texts.ts";

const props = defineProps<{ slot: string }>();
const emit = defineEmits<{ confirm: [name: string] }>();
const store = inject(STORE_KEY)!;
const id = useId();
const dialog = ref<HTMLDialogElement>();
const name = ref("");
const loading = ref(false);
const failed = ref(false);
let revision = 0;

function close(): void {
  revision++;
  dialog.value?.close();
}

async function open(): Promise<void> {
  if (!dialog.value || dialog.value.open) return;
  const current = ++revision;
  name.value = "";
  loading.value = true;
  failed.value = false;
  dialog.value.showModal();
  try {
    const reply = await api<{ name: string }>("GET", slotPath(props.slot, "network/invite-name"));
    if (current === revision && dialog.value?.open && !name.value) name.value = reply.name ?? "";
  } catch {
    if (current === revision) failed.value = true;
  } finally {
    if (current === revision) loading.value = false;
  }
}

function confirm(): void {
  if (loading.value || !dialog.value?.open) return;
  close();
  emit("confirm", name.value);
}

watch([() => props.slot, () => stateOf(store, props.slot)?.network?.id_hex], close);
defineExpose({ open });
</script>

<template>
  <dialog ref="dialog" class="member-dialog invite-name-dialog" :aria-labelledby="id + '-title'">
    <h2 :id="id + '-title'" class="member-dialog-title">Invite a member</h2>
    <form @submit.prevent="confirm">
      <label :for="id + '-name'" class="hint">{{ HOME_INVITE_FOR_LABEL }}</label>
      <input :id="id + '-name'" v-model="name" name="invite-name" type="text" class="input input-wide" maxlength="64" autocomplete="off" autofocus />
      <p v-if="failed" class="hint" role="status">Couldn't suggest a name. You can enter one here.</p>
      <div class="dialog-actions">
        <button type="button" class="btn" @click="close">Cancel</button>
        <button type="submit" class="btn btn-primary" :disabled="loading">Create invite</button>
      </div>
    </form>
  </dialog>
</template>
