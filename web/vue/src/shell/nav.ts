/* Navigation: seven sections, tab order and the address hash (a port of `05-nav.js`). */

import { SHELL_TAB_DOMAINS, SHELL_TAB_DOMAINS_NARROW, SHELL_TAB_HOME, SHELL_TAB_LOG, SHELL_TAB_NETWORK, SHELL_TAB_PEERS, SHELL_TAB_PORTS, SHELL_TAB_SETTINGS } from "./shell-texts.ts";

export type SectionId = "home" | "peers" | "domains" | "ports" | "network" | "log" | "settings";
export interface SectionView {
  id: SectionId;
  tab: string;
  title: string | null;
}

export const SECTION_VIEWS: SectionView[] = [
  { id: "home", tab: SHELL_TAB_HOME, title: null },
  { id: "peers", tab: SHELL_TAB_PEERS, title: SHELL_TAB_PEERS },
  { id: "domains", tab: SHELL_TAB_DOMAINS, title: SHELL_TAB_DOMAINS },
  { id: "ports", tab: SHELL_TAB_PORTS, title: SHELL_TAB_PORTS },
  { id: "network", tab: SHELL_TAB_NETWORK, title: SHELL_TAB_NETWORK },
  { id: "log", tab: SHELL_TAB_LOG, title: SHELL_TAB_LOG },
  { id: "settings", tab: SHELL_TAB_SETTINGS, title: SHELL_TAB_SETTINGS },
];

// Sections the phone app does not show.
const APP_HIDDEN: SectionId[] = ["domains", "ports", "network", "log"];

export const sectionViews = (inApp: boolean): SectionView[] => SECTION_VIEWS.filter((v) => !(inApp && APP_HIDDEN.includes(v.id)));

// Section identifiers in tab order.
export const SECTIONS: SectionId[] = SECTION_VIEWS.map((v) => v.id);

// A section by the address hash or `null`; the old `#overview` link is also "Home".
export function parseHash(hash: string | null | undefined): SectionId | null {
  const h = (hash || "").replace(/^#/, "");
  if (h === "overview") return "home";
  return SECTIONS.find((id) => id === h) ?? null;
}

// The section to open for an address hash: unknown and, in the phone app, hidden sections fall back to Home.
export function openableSection(hash: string | null | undefined, inApp: boolean): SectionId {
  const id = parseHash(hash);
  return sectionViews(inApp).find((v) => v.id === id)?.id ?? "home";
}

// Sections that live under the "More" menu on a narrow screen.
export const NARROW_MORE: SectionId[] = ["ports", "network", "log"];

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
