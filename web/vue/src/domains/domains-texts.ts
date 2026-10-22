// Texts of the "Site checks" section: marks, hints, table captions and the add form.

export const DOMAINS_VERDICT_OK = "opens";
export const DOMAINS_VERDICT_DNS_MISMATCH = "blocked: address spoofed";
export const DOMAINS_VERDICT_NETWORK_BLOCKED = "blocked by ISP";
export const DOMAINS_VERDICT_SITE_GEOBLOCK = "blocked for this country";
export const DOMAINS_VERDICT_ANTIBOT = "blocked as a bot";
export const DOMAINS_VERDICT_DOWN = "site is down";

export const DOMAINS_NOT_CHECKED = "not checked yet";
export const DOMAINS_UNKNOWN = "result unknown";

export function domainsCheckedAgoText(age: string): string {
  return "checked " + age;
}

export function domainsUnknownCodeText(verdict: string): string {
  return "check code: " + verdict;
}

export const DOMAINS_NOTE_OK = "the site will open through this member";
export const DOMAINS_NOTE_DNS_MISMATCH = "the ISP substitutes a wrong site address";
export const DOMAINS_NOTE_NETWORK_BLOCKED = "the ISP doesn't let you reach the site";
export const DOMAINS_NOTE_SITE_GEOBLOCK = "the site itself is closed to the member's country";
export const DOMAINS_NOTE_ANTIBOT = "may open in a browser after an \"I'm not a robot\" check";
export const DOMAINS_NOTE_DOWN = "the site is not responding";

export const DOMAINS_PEER_FALLBACK = "member";
export const DOMAINS_CHECKING_ALL = "Checking…";
export const DOMAINS_CHECK_ALL = "Check all";
export const DOMAINS_CHECK_ALL_CONFLICT = "A check is already running — the results will appear in the table.";
export const DOMAINS_ADD_HINT = "make sure the address looks like example.com";

export const DOMAINS_ACTION_ADD = "add the site";
export const DOMAINS_ACTION_RECHECK = "recheck ";
export const DOMAINS_ACTION_REMOVE = "remove ";
export const DOMAINS_ACTION_CHECK_ALL = "start the check";

export const DOMAINS_NO_STORE = "DomainsSection: store not provided (STORE_KEY)";
export const DOMAINS_LOADING = "Loading…";
export const DOMAINS_ADDRESS_LABEL = "Site address";
export const DOMAINS_ADDRESS_PLACEHOLDER = "site address, for example example.com";
export const DOMAINS_ADD = "Add";
export const DOMAINS_TABLE_LABEL = "Site checks";
export const DOMAINS_COL_SITE = "Site";
export const DOMAINS_COL_YOU = "For you";
export const DOMAINS_EMPTY = "No sites yet — enter an address above and click \"Add\"";
export const DOMAINS_HINT_INTRO = "Each member checks sites from their own internet connection, without";
export const DOMAINS_HINT_REST = "A green mark in a member's column means the site will open through them. The \"For you\" column shows whether the site opens for you as usual.";
export const DOMAINS_HINT_LOCAL = "The site list is stored only on this computer: other members can't see or change it.";

export const DOMAINS_CHECKING_ROW = "checking…";
export const DOMAINS_CHOSEN_PREFIX = "✓ Your exit:";
export const DOMAINS_RECHECK = "Check again";
export const DOMAINS_REMOVE = "Remove";
export const DOMAINS_YOUR_EXIT = "✓ Your exit";
export const DOMAINS_NOT_CHECKED_CARD = "— not checked yet";

export function domainsAllSitesViaText(via: string): string {
  return "All sites will go through " + via;
}

export function domainsRecheckLabelText(name: string): string {
  return "Recheck " + name;
}

export function domainsRemoveLabelText(name: string): string {
  return "Remove " + name;
}
