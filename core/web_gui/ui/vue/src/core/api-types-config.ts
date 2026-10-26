/* Node API types: config `GET`/`PUT /api/config`, backup. Types only; the shared header and rules are in `api-types.ts`. */

import type { ConfigFileState, PortProto } from "./api-types-state.ts";

// ---------------------------------------------------------------- config

/** The `config` document in the `GET`/`PUT /api/config` responses and in 409 — docs/WEB-API-config.md §2.
 * Always the values from the file, not the effective ones (launch flags are not mixed in). */
export interface ConfigDocument {
  version: number;
  node: { name: string | null }; // null — no name in the file
  network: ConfigNetworkView; // read-only
  dht: { bootstrap: string[] };
  exit: { enabled: boolean };
  socks: { enabled: boolean; port: number; access: "localhost" | "localnet"; ifaces: string[] }; // ifaces: [] — automatic, ["*"] — all networks
  checks: {
    enabled: boolean;
    interval_sec: number;
    doh_server: string; // "address:port"
    provider_dns: string | null;
  };
  probe: { interval_sec: number };
  route: { exits: string[] }; // full ids, the order is the priority
  ports: { published: PublishedPortConfig[]; opened: OpenedPortConfig[] };
  web: { enabled: boolean; port: number; access: "localhost" | "localnet"; ifaces: string[] }; // access and ifaces are editable, port and enabled are read-only
  log: { ring_lines: number };
  autostart: UnavailableFeature;
  full_tunnel: UnavailableFeature;
}

/** `config.network`: without the key — empty strings, the whole key is never in the document. */
interface ConfigNetworkView {
  label: string;
  masked: string;
}

/** Own port publication in the config; `for` — `"all"` or a list of full ids. */
export interface PublishedPortConfig {
  port: number;
  proto?: PortProto; // absent means "tcp"
  name: string;
  for: "all" | string[];
}

/** Another member's port opened here: `node` — full id, `local` — the listener port. */
export interface OpenedPortConfig {
  node: string;
  port: number;
  proto?: PortProto; // absent means "tcp"
  local: number;
}

/** `autostart`/`full_tunnel`: `available` and `reason` are inserted by the node, they are not in the file. */
export interface UnavailableFeature {
  enabled: boolean;
  available: boolean;
  reason: string;
}

/** Dotted path of a config field: `"socks.port"`, `"route.exits"`… */
type ConfigPath = string;

/** Error of a single field: in `file.field_errors`, in `fields` of the 422 responses. */
export interface FieldError {
  path: ConfigPath;
  message: string;
}

/** The `file` block — the state of the settings file (`ConfigState::file_block`) — docs/WEB-API-config.md §1.6. */
export interface ConfigFileInfo {
  path: string | null; // null — the node runs without a file (--no-config)
  state: ConfigFileState;
  writable: boolean;
  message: string | null;
  field_errors: FieldError[];
}

/** `GET /api/config` — docs/WEB-API-config.md §1.6. */
export interface ConfigResponse {
  rev: number;
  config: ConfigDocument;
  read_only: ConfigPath[];
  restart_fields: ConfigPath[];
  restart_pending: ConfigPath[];
  cli_overrides: ConfigPath[];
  file: ConfigFileInfo;
}

/** Config meta lists in the page store (`store.configMeta`, `core/save-config.js`). */
export type ConfigMeta = Pick<ConfigResponse, "read_only" | "restart_fields" | "restart_pending" | "cli_overrides">;
