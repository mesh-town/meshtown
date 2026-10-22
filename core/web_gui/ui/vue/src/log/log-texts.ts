/* Texts of the "Log" section: buttons, captions, the hint about the line ring, polling messages. */

import { NET_NAME } from "../common/words.ts";

export const LOG_STORE_MISSING = "LogSection: store not provided (STORE_KEY)";
export const LOG_TAG_ALL = "all";
export const LOG_SEARCH_PLACEHOLDER = "search in lines";
export const LOG_AUTOSCROLL = "Auto-scroll";
export const LOG_RESUME = "Resume";
export const LOG_PAUSE = "Pause";
export const LOG_DOWNLOAD = "Download .log";
export const LOG_WINDOW_LABEL = "Node log";
export const LOG_LOADING = "Loading log…";
export const LOG_GAP_TEXT = "⋯ lines skipped here: " + NET_NAME + " erased them before the page could read them ⋯";
export const LOG_RING_HINT_TAIL = " lines. It starts over when " + NET_NAME + " restarts. To keep it, click \"Download .log\".";
export const LOG_RING_HINT_HEAD = "The log is kept only in memory: the last ";
export const LOG_POLL_FAILED_PREFIX = "Couldn't update the log: ";
export const LOG_POLL_FAILED_RETRY = "Couldn't update the log — try again";
