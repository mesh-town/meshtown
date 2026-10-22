/* Summary for help: a whitelist of fields (version, status-light rows, log lines). Log
 * lines pass through `redactLine`: the token, network and node ids and addresses are cut out. The mask, key,
 * invite and `id_full` from the snapshot are not read here. */

import type { LogLine, NodeState } from "../core/api-types.ts";
import { formatLogTime } from "../log/log-view.ts";
import type { CheckRow, DiagState } from "./diag-checks.ts";
import { LEVEL_ICON } from "./diag-checks.ts";
import { redactLine } from "./diag-redact.ts";
import { DIAG_LOG_UNAVAILABLE_PREFIX, DIAG_SUMMARY_LOG_HEADER } from "./diag-texts.ts";

const LOG_LINES = 50;

export function summaryLogPath(state: DiagState): string | null {
  const log = state && state.log;
  if (!log) return null;
  const after = Math.max(0, (log.last_seq || 0) - LOG_LINES);
  return "/api/log?after=" + after + "&limit=" + LOG_LINES;
}

const checkLine = (c: CheckRow): string => LEVEL_ICON[c.level] + " " + c.text;
const logLine = (e: LogLine): string => formatLogTime(e.t) + " [" + e.tag + "] " + e.text;

// Snapshot values that must not remain in the log lines.
function secretValues(state: DiagState): (string | undefined)[] {
  const node: Partial<NodeState["node"]> = (state && state.node) || {};
  const net: Partial<NonNullable<NodeState["network"]>> = (state && state.network) || {};
  return [net.id_hex, node.id, node.id_full];
}

function journalPart(lines: LogLine[] | undefined, logError: string | undefined): string[] {
  if (logError) return [DIAG_LOG_UNAVAILABLE_PREFIX + logError];
  return (lines || []).map(logLine);
}

export interface SummaryInput {
  state: DiagState;
  checks: CheckRow[];
  lines?: LogLine[];
  logError?: string;
}

export function buildSummary({ state, checks, lines, logError }: SummaryInput): string {
  const node: Partial<NodeState["node"]> = (state && state.node) || {};
  const secrets = secretValues(state);
  return [
    "Mesh Town " + (node.version || "—"),
    ...checks.map(checkLine),
    "",
    DIAG_SUMMARY_LOG_HEADER,
    ...journalPart(lines, logError).map((l) => redactLine(l, secrets)),
  ].join("\n");
}
