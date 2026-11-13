// "Copy invite" and "Show QR code" of a pending invite: the kept string is fetched on demand, used at once (clipboard or
// the share window) and not stored beyond the open window; closing the window forgets it.
import { ref } from "vue";
import { api } from "../core/api.ts";
import { writeClipboard } from "../common/clipboard.ts";
import { defaultHomeDeps } from "../home/home-actions.ts";
import { qrImageSrc } from "../home/invite-actions.ts";
import { getInviteText, inviteErrorText } from "./invites-api.ts";
import type { OpenInvite } from "./invites-api.ts";

export interface Shown {
  name: string;
  text: string;
  qr: string;
}

export function useKeptInvite(slot: () => string) {
  const copied = ref<Record<string, "done" | "manual">>({});
  const failed = ref<Record<string, string>>({});
  const shown = ref<Shown | null>(null);

  const note = (hash: string, text: string): void => {
    failed.value = { ...failed.value, [hash]: text };
  };

  async function fetchText(i: OpenInvite) {
    note(i.hash, "");
    try {
      const reply = await getInviteText(api, slot(), i.hash);
      if (!reply) note(i.hash, inviteErrorText(null));
      return reply;
    } catch (err) {
      note(i.hash, inviteErrorText(err));
      return null;
    }
  }

  async function copy(i: OpenInvite): Promise<void> {
    const reply = await fetchText(i);
    if (!reply) return;
    const done = await writeClipboard(defaultHomeDeps, reply.invite);
    copied.value = { ...copied.value, [i.hash]: done ? "done" : "manual" };
  }

  // Returns true when the window should open.
  async function showQr(i: OpenInvite): Promise<boolean> {
    const reply = await fetchText(i);
    if (reply) shown.value = { name: i.for_whom ?? "", text: reply.invite, qr: qrImageSrc(reply.qr ?? "") };
    return reply !== null;
  }

  const forget = (): void => {
    shown.value = null;
  };

  return { copied, failed, shown, copy, showQr, forget };
}
