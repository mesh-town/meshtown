<script setup lang="ts">
// "Pending invites": the open invites of a managed network, for admins only. A label ("for whom") is an admin's note;
// the admin who made the invite edits it in place, for the others it is read-only.
import { ref, watch } from "vue";
import { api } from "../core/api.ts";
import { agoText } from "../common/words.ts";
import { getInvites, inviteErrorText, relabelInvite } from "./invites-api.ts";
import type { OpenInvite } from "./invites-api.ts";
import { INVITES_EMPTY, INVITES_NO_NAME, INVITES_SAVE, INVITES_TITLE, invitedBy, issuedAgo, labelAria } from "./invites-texts.ts";

const props = defineProps<{ slot: string; meAdmin: boolean; refreshKey?: string | number }>();

const invites = ref<OpenInvite[]>([]);
const drafts = ref<Record<string, string>>({});
const errors = ref<Record<string, string>>({});
const loaded = ref(false);

const draftOf = (i: OpenInvite): string => drafts.value[i.hash] ?? i.for_whom ?? "";
const dirty = (i: OpenInvite): boolean => draftOf(i).trim() !== (i.for_whom ?? "");
const ago = (i: OpenInvite): string => issuedAgo(agoText(Math.max(0, Math.floor(Date.now() / 1000) - i.made_unix)));

async function load(): Promise<void> {
  if (!props.meAdmin) return;
  try {
    invites.value = (await getInvites(api, props.slot)) ?? [];
  } catch {
    // The list is a courtesy: a failed read keeps the last one.
  }
  loaded.value = true;
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

watch(() => [props.slot, props.meAdmin, props.refreshKey], load, { immediate: true });
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
          <span v-if="errors[i.hash]" class="field-error-text" role="alert">{{ errors[i.hash] }}</span>
        </span>
        <span class="peer-meta"><span class="peer-line">{{ ago(i) }}</span><span class="peer-line">{{ invitedBy(i.mine, i.issuer_name) }}</span></span>
      </li>
    </ul>
  </section>
</template>
