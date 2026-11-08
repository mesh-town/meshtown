// Texts of the "Site checks" section: marks, hints, table captions and the add form.

export const DOMAINS_VERDICT_OK = "opens";
export const DOMAINS_VERDICT_DNS_MISMATCH = "blocked by provider (wrong address)";
export const DOMAINS_VERDICT_NETWORK_BLOCKED = "blocked by provider";
export const DOMAINS_VERDICT_SITE_GEOBLOCK = "blocked for this country";
export const DOMAINS_VERDICT_ANTIBOT = "robot check";
export const DOMAINS_VERDICT_DOWN = "site is down";

export const DOMAINS_NOT_CHECKED = "not checked yet";
export const DOMAINS_UNKNOWN = "result unknown";

export function domainsCheckedAgoText(age: string): string {
  return "checked " + age;
}

export function domainsUnknownCodeText(verdict: string): string {
  return "check code: " + verdict;
}

export const DOMAINS_NOTE_OK = "the site opens through this exit";
export const DOMAINS_NOTE_DNS_MISMATCH = "your internet provider answers with a wrong address for the site";
export const DOMAINS_NOTE_NETWORK_BLOCKED = "your internet provider doesn't let the connection through";
export const DOMAINS_NOTE_SITE_GEOBLOCK = "the site itself is closed to the member's country";
export const DOMAINS_NOTE_ANTIBOT = "the site asks to prove you're not a robot; usually opens in the browser";
export const DOMAINS_NOTE_DOWN = "the site is not responding";
export const DOMAINS_VERDICT_HERE = "doesn't open from here";
export const DOMAINS_NOTE_HERE = "the site opens for a member but not for you";
export const DOMAINS_TITLE_HERE = "The site answers other members but not you: your provider or the site itself blocks your connection.";

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
export const DOMAINS_COL_YOU_SUB = "no exit chosen";
export const DOMAINS_COL_VIA_SUB = "your exit";
export const DOMAINS_COL_OTHERS = "Other exits";

export function domainsViaHeadText(name: string): string {
  return "Via " + name;
}

/** The "Other exits" line in two parts: plain text (`lead`) and the clickable counter (`more`, may be empty). */
export interface OthersText {
  lead: string;
  more: string;
}

export function domainsMoreText(n: number): string {
  return n + " more";
}

export function domainsOpensViaText(name: string, more: number): OthersText {
  return more > 0 ? { lead: "opens via " + name + " and", more: domainsMoreText(more) } : { lead: "opens via " + name, more: "" };
}

export function domainsAlsoViaText(n: number): OthersText {
  return n > 0 ? { lead: "also via", more: domainsMoreText(n) } : { lead: "\u2014", more: "" };
}

export const DOMAINS_ALT_NONE = "no other exit opens it";
export const DOMAINS_ALT_UNCHECKED = "other exits not checked yet";
export const DOMAINS_EXIT_OFFLINE = "your exit is offline";
export const DOMAINS_EXIT_NOT_SHARING = "your exit stopped sharing internet";
export const DOMAINS_LEGEND_TITLE = "What the marks mean";
export const DOMAINS_DETAILS = "Details";
export const DOMAINS_DETAILS_HIDE = "Hide";
export const DOMAINS_ROW_DIRECT = "This computer, directly";
export const DOMAINS_EMPTY = "No sites yet — enter an address above and click \"Add\"";
export const DOMAINS_HINT_INTRO = "Checks run on their own every few minutes: from this computer and through every member who shares their internet.";
export const DOMAINS_NO_EXIT = "No member shares their internet yet \u2014 sites are checked only from your connection.";
export const DOMAINS_HINT_LOCAL = "The site list stays on this computer; other members can't see it.";

export const DOMAINS_CHECKING_ROW = "checking…";
export const DOMAINS_RECHECK = "Check again";
export const DOMAINS_REMOVE = "Remove";
export const DOMAINS_YOUR_EXIT = "✓ Your exit";
export const DOMAINS_NOT_CHECKED_CARD = "— not checked yet";

export function domainsExitSavedText(name: string): string {
  return "Done \u2014 sites now open via " + name + ".";
}

export function domainsAllSitesViaText(via: string): string {
  return "All sites will go through " + via;
}

export function domainsRecheckLabelText(name: string): string {
  return "Recheck " + name;
}

export function domainsRemoveLabelText(name: string): string {
  return "Remove " + name;
}
