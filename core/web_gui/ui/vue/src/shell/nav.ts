/* Navigation: six sections, tab order and the address hash. */

import type { NetSub } from "./net-subs.ts";
import { SHELL_TAB_DOMAINS, SHELL_TAB_DOMAINS_NARROW, SHELL_TAB_HOME, SHELL_TAB_LOG, SHELL_TAB_NETWORK, SHELL_TAB_PEERS, SHELL_TAB_SETTINGS } from "./shell-texts.ts";

export type SectionId = "home" | "peers" | "domains" | "network" | "log" | "settings";
interface SectionView {
  id: SectionId;
  tab: string;
  title: string | null;
}

export const SECTION_VIEWS: SectionView[] = [
  { id: "home", tab: SHELL_TAB_HOME, title: null },
  { id: "peers", tab: SHELL_TAB_PEERS, title: SHELL_TAB_PEERS },
  { id: "domains", tab: SHELL_TAB_DOMAINS, title: SHELL_TAB_DOMAINS },
  { id: "network", tab: SHELL_TAB_NETWORK, title: SHELL_TAB_NETWORK },
  { id: "log", tab: SHELL_TAB_LOG, title: SHELL_TAB_LOG },
  { id: "settings", tab: SHELL_TAB_SETTINGS, title: SHELL_TAB_SETTINGS },
];

// Sections the phone app does not show.
const APP_HIDDEN: SectionId[] = ["domains", "network", "log"];

export const sectionViews = (inApp: boolean): SectionView[] => SECTION_VIEWS.filter((v) => !(inApp && APP_HIDDEN.includes(v.id)));

// Section identifiers in tab order.
export const SECTIONS: SectionId[] = SECTION_VIEWS.map((v) => v.id);

// A section by the address hash or `null`; the old `#overview` link is also "Home", the old `#ports` link is "Networks".
export function parseHash(hash: string | null | undefined): SectionId | null {
  const h = (hash || "").replace(/^#/, "");
  if (h === "overview") return "home";
  if (h === "ports") return "peers";
  if (h.startsWith("peers/")) return "peers";
  return SECTIONS.find((id) => id === h) ?? null;
}

const PEERS_TAIL = /^peers\/(?:(add)|(main|[a-z2-7]{6})(?:\/(?:members|services|device))?)$/;

// The tab named by `#peers/<tab>[/<sub>]`: `main`, a six-character slot id or `add`; anything else — `null`.
export function peersTail(hash: string | null | undefined): string | null {
  const m = PEERS_TAIL.exec((hash || "").replace(/^#/, ""));
  return m ? (m[1] ?? m[2]) : null;
}

// The address hash of the Peers section with a tab and a sub-tab; no tab — the bare section, "members" writes no tail.
export function peersHash(tab: string | null, sub: NetSub = "members"): string {
  if (tab === null) return "#peers";
  return sub === "members" || tab === "add" ? `#peers/${tab}` : `#peers/${tab}/${sub}`;
}

// The tab to show: "add" always stands; a wanted visible slot; otherwise the first visible one.
export function pickPeersTab(wanted: string | null, visible: string[]): string | null {
  if (wanted === "add") return "add";
  if (wanted !== null && visible.includes(wanted)) return wanted;
  return visible[0] ?? null;
}

// The section to open for an address hash: unknown and, in the phone app, hidden sections fall back to Home.
export function openableSection(hash: string | null | undefined, inApp: boolean): SectionId {
  const id = parseHash(hash);
  return sectionViews(inApp).find((v) => v.id === id)?.id ?? "home";
}

// Sections that live under the "More" menu on a narrow screen.
export const NARROW_MORE: SectionId[] = ["network", "log"];

// Splits the tab list into the ordinary tabs and the ones under "More".
export function splitTabs(views: SectionView[]): { main: SectionView[]; more: SectionView[] } {
  return {
    main: views.filter((v) => !NARROW_MORE.includes(v.id)),
    more: views.filter((v) => NARROW_MORE.includes(v.id)),
  };
}

// The tab text and heading of a section; "Site checks" is shortened to "Sites" on a narrow screen.
export const viewTab = (v: SectionView, narrow: boolean): string => (narrow && v.id === "domains" ? SHELL_TAB_DOMAINS_NARROW : v.tab);
export const viewTitle = (v: SectionView, narrow: boolean): string | null => (v.title === null ? null : viewTab({ ...v, tab: v.title }, narrow));
