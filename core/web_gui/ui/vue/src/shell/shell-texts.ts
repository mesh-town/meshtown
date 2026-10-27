// Shell texts: tabs and section titles, connection strips, tab captions.

import { NET_NAME } from "../common/words.ts";
import { inAppPage } from "../home/browser-setup.ts";

export const SHELL_NO_STORE = "AppShell: store not provided (STORE_KEY)";

export const SHELL_TAB_HOME = "Home";
export const SHELL_TAB_PEERS = "Networks";
export const SHELL_TAB_DOMAINS = "Site checks";
export const SHELL_TAB_DOMAINS_NARROW = "Sites";
export const SHELL_TAB_MORE = "More";
export const SHELL_TAB_NETWORK = "Diagnostics";
export const SHELL_TAB_LOG = "Log";
export const SHELL_TAB_SETTINGS = "Settings";

export const SHELL_RESTARTED = NET_NAME + " is responding again";
export const SHELL_BAD_ORIGIN = "The page was opened from the wrong address — open it from the " + NET_NAME + " icon near the clock";
export const SHELL_BAD_ORIGIN_APP = "The page was opened from the wrong address — open it from the " + NET_NAME + " app";
export const shellBadOriginText = (): string => (inAppPage() ? SHELL_BAD_ORIGIN_APP : SHELL_BAD_ORIGIN);
export const SHELL_STOPPED = NET_NAME + " is stopped — start it: the " + NET_NAME + " icon near the clock → \"Start " + NET_NAME + "\". You don't need to refresh the page.";
export const SHELL_STOPPED_APP = NET_NAME + " is stopped — start it: open the " + NET_NAME + " app. You don't need to refresh the page.";
export const SHELL_CHANGED = "Settings were changed in another tab or by " + NET_NAME + " itself";

export const SHELL_RELOAD = "Reload";
export const SHELL_TOKEN_URL_PLACEHOLDER = "page link";
export const SHELL_TOKEN_URL_APPLY = "Open";
export const SHELL_TAB_UNSAVED = "You have unsaved changes";

// The node identity strip: the Rust test `identity_texts_match_page` reads these phrases verbatim.
export const IDENTITY_DECLINED_NOTE = NET_NAME + " was started in a mode that doesn't remember this computer. ";
export const IDENTITY_PROBLEM_NOTE = NET_NAME + " couldn't remember this computer: there is an error in the settings file. ";
export const IDENTITY_PRESERVED_NOTE = "The settings file couldn't be updated, and " + NET_NAME + " left it as is so that nothing is lost. Don't delete this file: it may contain access to your " + NET_NAME + " network. ";
export const restartConsequenceText = (who: string): string => "After a restart, other members will see " + who + " as a new member.";
