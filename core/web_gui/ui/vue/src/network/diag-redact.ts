/* Cleaning a log line before it gets into the summary for help: the page token, the network and node ids,
 * addresses. A pure function without Vue and DOM. */

import { DIAG_HIDDEN_ADDR, DIAG_HIDDEN_TOKEN } from "./diag-texts.ts";

const TOKEN = /token=[^\s&)"'<>]+/gi;
const IPV4 = /(?<![\w.])\d{1,3}(?:\.\d{1,3}){3}(?::\d+)?(?![\w.])/g;
const IPV6_BRACKET = /\[[0-9a-f:.]*:[0-9a-f:.]*\](?::\d+)?/gi;
const IPV6_FULL = /(?<![\w:])(?:[0-9a-f]{1,4}:){7}[0-9a-f]{1,4}(?![\w:])/gi;
// The full node id or a key in hex — any long hex, including peers' ids.
const HEX_LONG = /(?<![0-9a-f])[0-9a-f]{40,}(?![0-9a-f])/gi;
const IPV6_SHORT = /(?<![\w:])(?=[0-9a-f:]*::)(?=[0-9a-f:]*[0-9a-f])[0-9a-f:]{3,}(?![\w:])/gi;

export const HIDDEN_TOKEN = DIAG_HIDDEN_TOKEN;
export const HIDDEN_ID = "‹id›";
export const HIDDEN_ADDR = DIAG_HIDDEN_ADDR;

// Long values go first: a short node id is the beginning of the full one and would otherwise leave its tail.
function hideValues(line: string, values: string[]): string {
  const longFirst = [...values].sort((a, b) => b.length - a.length);
  return longFirst.reduce((text, v) => text.split(v).join(HIDDEN_ID), line).replace(HEX_LONG, HIDDEN_ID);
}

// values — the network and node ids from the snapshot (full and short); empty ones are skipped.
export function redactLine(line: unknown, values: readonly unknown[] = []): string {
  const known = values.filter((v): v is string => typeof v === "string" && v.length > 0);
  const noToken = String(line).replace(TOKEN, HIDDEN_TOKEN);
  return hideValues(noToken, known)
    .replace(IPV6_BRACKET, HIDDEN_ADDR)
    .replace(IPV6_FULL, HIDDEN_ADDR)
    .replace(IPV6_SHORT, HIDDEN_ADDR)
    .replace(IPV4, HIDDEN_ADDR);
}
