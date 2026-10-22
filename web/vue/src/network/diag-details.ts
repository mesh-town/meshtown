/* Diagnostics details without Vue: the connection table, "About", the latest events,
 * the text of the "Connect manually" response. */

import type {
  DhtInfo, LogLine, NodeRuntime, PeerAddressesAddResponse, RecordInfo, TransportInfo,
} from "../core/api-types.ts";
import {
  DIAG_DHT_CONNECTED, DIAG_DHT_NONE, DIAG_RECORD_NONE, DIAG_RECORD_PUBLISHED, DIAG_ROW_DIRECTORY, DIAG_ROW_PORT, DIAG_ROW_RECORD,
  diagAboutHeadText, diagAboutMemoryText, diagDhtNodesText, diagDhtRefreshedText, diagPeerAddedText, diagRecordTtlText,
} from "./diag-texts.ts";
import { formatLogTime } from "../log/log-view.ts";
import { formatMib, formatUptime } from "./diag-checks.ts";

const DASH = "—";

function dhtText(dht: DhtInfo | null | undefined): string {
  if (!dht || !dht.joined) return DIAG_DHT_NONE;
  const tail = dht.last_publish_sec == null
    ? ""
    : diagDhtRefreshedText(dht.last_publish_sec);
  const nodes = dht.nodes > 0 ? diagDhtNodesText(dht.nodes) : "";
  return DIAG_DHT_CONNECTED + nodes + tail;
}

function recordText(record: RecordInfo | null | undefined): string {
  if (!record || !record.published) return DIAG_RECORD_NONE;
  if (record.ttl_sec == null) return DIAG_RECORD_PUBLISHED;
  return diagRecordTtlText(record.ttl_sec);
}

export interface TransportRow {
  label: string;
  value: string;
  mono?: boolean;
}

export interface EventRow {
  seq: number;
  time: string;
  tag: string;
  text: string;
}

export function transportRows(transport: Partial<TransportInfo> | null | undefined): TransportRow[] {
  const t: Partial<TransportInfo> = transport || {};
  return [
    { label: "NAT", value: t.nat || DASH },
    { label: DIAG_ROW_PORT, value: (t.listen_udp || []).join(", "), mono: true },
    { label: DIAG_ROW_DIRECTORY, value: dhtText(t.dht) },
    { label: DIAG_ROW_RECORD, value: recordText(t.record) },
  ];
}

export function aboutLines(node: Partial<NodeRuntime> | null | undefined): string[] {
  const n: Partial<NodeRuntime> = node || {};
  return [
    diagAboutHeadText(n.web_addr || DASH, n.version || DASH),
    diagAboutMemoryText(formatMib(n.rss_kib), formatUptime(n.uptime_sec)),
  ];
}

export function eventRows(tail: LogLine[] | null | undefined): EventRow[] {
  if (!Array.isArray(tail)) return [];
  return tail.map((e) => ({ seq: e.seq, time: formatLogTime(e.t), tag: e.tag, text: e.text }));
}

export function peerAddrResultText(res: Partial<PeerAddressesAddResponse> | null | undefined): string {
  const rejected = (res && res.rejected) || [];
  const reasons = rejected.map((r) => r.line + ": " + r.message);
  const tail = reasons.length ? " (" + reasons.join("; ") + ")" : "";
  const added = (res && res.added) ?? 0;
  return diagPeerAddedText(added, rejected.length, tail);
}
