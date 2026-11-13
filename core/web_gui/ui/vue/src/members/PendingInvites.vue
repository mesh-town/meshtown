<script setup lang="ts">
// "Pending invites": the open invites of a managed network, for admins only. A label ("for whom") is an admin's note;
// the admin who made the invite edits it in place, for the others it is read-only.
import { nextTick, ref, watch } from "vue";
import { api } from "../core/api.ts";
import { INVITE_SHARE_COPIED } from "../home/invite-share-texts.ts";
import InviteShareDialog from "../home/InviteShareDialog.vue";
import InviteShareIcon from "../home/InviteShareIcon.vue";
import { getInvites, inviteErrorText, relabelInvite } from "./invites-api.ts";
import type { OpenInvite } from "./invites-api.ts";
import { INVITES_COPY, INVITES_EMPTY, INVITES_NO_NAME, INVITES_SAVE, INVITES_SHOW_QR, INVITES_TITLE, invitedBy, issuedAt, labelAria } from "./invites-texts.ts";
import { useKeptInvite } from "./use-kept-invite.ts";

const props = defineProps<{ slot: string; meAdmin: boolean; refreshKey?: string | number; issueRevision?: number }>();

const invites = ref<OpenInvite[]>([]);
const drafts = ref<Record<string, string>>({});
const errors = ref<Record<string, string>>({});
const loaded = ref(false);
const kept = useKeptInvite(() => props.slot);
const shareDialog = ref<InstanceType<typeof InviteShareDialog>>();

async function openWindow(opened: boolean, mode: "text" | "qr"): Promise<void> {
  if (!opened) return;
  await nextTick();
  shareDialog.value?.open(mode);
}

const showQr = async (i: OpenInvite): Promise<void> => openWindow(await kept.showQr(i), "qr");
const copyInvite = async (i: OpenInvite): Promise<void> => openWindow(await kept.copy(i), "text");

const draftOf = (i: OpenInvite): string => drafts.value[i.hash] ?? i.for_whom ?? "";
const dirty = (i: OpenInvite): boolean => draftOf(i).trim() !== (i.for_whom ?? "");
let loadRevision = 0;

async function load(): Promise<void> {
  const revision = ++loadRevision;
  if (!props.meAdmin) return;
  try {
    const reply = await getInvites(api, props.slot);
    if (revision === loadRevision) invites.value = reply ?? [];
  } catch {
    // Keep the last list if the read fails.
  }
  if (revision === loadRevision) loaded.value = true;
}

async function save(i: OpenInvite): Promise<void> {
  errors.value = { ...errors.value, [i.hash]: "" };
  try {
    invites.value = (await relabelInvite(api, props.slot, i.hash, draftOf(i))) ?? invites.value;
    drafts.value = {};
  } catch (err) {
    errors.value = { ...errors.value, [i.hash]: inviteErrorText(err) };
  }
}

watch(() => [props.slot, props.meAdmin, props.refreshKey, props.issueRevision], load, { immediate: true });
</script>

<template>
  <section v-if="meAdmin" id="pending-invites" class="card" role="region" aria-labelledby="pending-invites-title">
    <h2 id="pending-invites-title">{{ INVITES_TITLE }}</h2>
    <p v-if="loaded && invites.length === 0" class="hint" id="pending-invites-empty">{{ INVITES_EMPTY }}</p>
    <ul v-else class="peer-list" role="list">
      <li v-for="i in invites" :key="i.hash" class="peer-row invite-row" :data-invite="i.hash">
        <span class="peer-who">
          <form v-if="i.mine" class="invite-edit" @submit.prevent="save(i)">
            <input class="input" type="text" maxlength="64" :value="draftOf(i)" :placeholder="INVITES_NO_NAME" :aria-label="labelAria" @input="drafts[i.hash] = ($event.target as HTMLInputElement).value">
            <button v-if="dirty(i)" type="submit" class="btn btn-sm">{{ INVITES_SAVE }}</button>
          </form>
          <span v-else class="peer-name" :class="{ 'invite-unnamed': !i.for_whom }">{{ i.for_whom ?? INVITES_NO_NAME }}</span>
          <span v-if="errors[i.hash] || kept.failed.value[i.hash]" class="field-error-text" role="alert">{{ errors[i.hash] || kept.failed.value[i.hash] }}</span>
          <span v-if="i.kept" class="invite-share-links">
            <button type="button" class="invite-share-link" data-act="copy" @click="copyInvite(i)"><InviteShareIcon kind="copy" />{{ INVITES_COPY }}</button>
            <button type="button" class="invite-share-link" data-act="qr" @click="showQr(i)"><InviteShareIcon kind="qr" />{{ INVITES_SHOW_QR }}</button>
            <span v-if="kept.copied.value[i.hash]" class="hint" role="status">{{ INVITE_SHARE_COPIED }}</span>
          </span>
        </span>
        <span class="peer-meta">
          <time class="peer-line" :datetime="new Date(i.made_unix * 1000).toISOString()">{{ issuedAt(i.made_unix) }}</time>
          <span class="peer-line">{{ invitedBy(i.mine, i.issuer_name) }}</span>
        </span>
      </li>
    </ul>
    <InviteShareDialog ref="shareDialog" :text="kept.shown.value?.text ?? ''" :qr="kept.shown.value?.qr ?? ''" :name="kept.shown.value?.name ?? ''" warning="" @closed="kept.forget" />
  </section>
</template>
