/* Protocol helpers of the "Ports" section: a missing `proto` means TCP, the key of a port, the member-side error text. */

import type { NetPort, PortProto } from "../core/api-types.ts";
import { PORTS_LAST_ERROR_TEXT } from "./ports-texts.ts";

export function protoOf(x: { proto?: PortProto }): PortProto {
  return x.proto ?? "tcp";
}

// TCP — "8080", UDP — "53/udp".
export function portKey(proto: PortProto, port: number): string {
  return proto === "udp" ? port + "/udp" : String(port);
}

// "" — nothing to show; an old member answers TCP rows with `unsupported`, which reads as a plain failure.
export function lastErrorText(row: NetPort): string {
  if (row.opened == null || row.opened.last_error == null) return "";
  const err = row.opened.last_error;
  return PORTS_LAST_ERROR_TEXT[err === "unsupported" && protoOf(row) === "tcp" ? "failed" : err];
}
