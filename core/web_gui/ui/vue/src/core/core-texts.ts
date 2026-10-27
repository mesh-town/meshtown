// Texts of the page core: node request errors, the save caption, bus service messages.

import { NET_NAME } from "../common/words.ts";
import { inAppPage } from "../home/browser-setup.ts";

export const CORE_API_TIMEOUT = NET_NAME + " didn't respond in time";
export const CORE_API_BAD_TOKEN = "The page was opened from an old or incomplete link — open it again: the " + NET_NAME + " icon near the clock → \"Open " + NET_NAME + "…\". Or paste the new link:";
export const CORE_API_BAD_TOKEN_APP = "The page was opened from an old or incomplete link — open it again from the " + NET_NAME + " app. Or paste the new link:";
export const coreApiBadTokenText = (): string => (inAppPage() ? CORE_API_BAD_TOKEN_APP : CORE_API_BAD_TOKEN);
export const CORE_API_NO_CONFIG = "Settings haven't been loaded yet";

export function coreApiHttpText(status: number): string {
  return NET_NAME + " responded with an error (code " + status + ")";
}

export function coreBridgeUnknownWhatText(what: string): string {
  return "meshtown:changed: unknown what " + what;
}

export const CORE_SAVED = "Saved";
export const CORE_SAVE_FAILED_RETRY = "Couldn't save — try again";

export function coreSaveFailedText(reason: string): string {
  return "Couldn't save: " + reason;
}
