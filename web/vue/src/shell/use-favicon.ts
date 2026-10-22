/* The live tab icon: redrawn when the ring composition changes (roles and counters), not on every poll. */

import { watch } from "vue";
import type { Store } from "../core/store.ts";
import { faviconHref, faviconSvg } from "../home/ring-favicon.ts";
import { ringMembers } from "../home/ring-members.ts";

const DARK_QUERY = "(prefers-color-scheme: dark)";

export function compositionKey(members: Array<{ role: string }>): string {
  const counts: Record<string, number> = {};
  for (const m of members) counts[m.role] = (counts[m.role] || 0) + 1;
  return ["via", "client", "online", "offline"].map((r) => `${r}${counts[r] || 0}`).join(",");
}

function iconLink(): HTMLLinkElement {
  const found = document.querySelector<HTMLLinkElement>("link[rel~='icon']");
  if (found) return found;
  const link = document.createElement("link");
  link.rel = "icon";
  document.head.appendChild(link);
  return link;
}

export function useFavicon(store: Store): void {
  const media = typeof matchMedia === "function" ? matchMedia(DARK_QUERY) : null;
  const apply = (): void => {
    iconLink().href = faviconHref(faviconSvg(ringMembers(store.state), media !== null && media.matches));
  };
  watch(() => compositionKey(ringMembers(store.state)), apply, { immediate: true });
  media?.addEventListener?.("change", apply);
}
