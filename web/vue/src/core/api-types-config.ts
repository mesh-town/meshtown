/* Node API types: config `GET`/`PUT /api/config`, backup. Types only; the shared header and rules are in `api-types.ts`. */

import type { ConfigFileState, TransportMode } from "./api-types-state.ts";

// ---------------------------------------------------------------- config

/** The `config` document in the `GET`/`PUT /api/config` responses and in 409 — WEB-API-config.md §2.
 * Always the values from the file, not the effective ones (launch flags are not mixed in). */
export interface ConfigDocument {
  version: number;
  node: { name: string | null }; // null — no name in the file
  network: ConfigNetworkView; // read-only
  transport: {
    mode: TransportMode;
  };
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
export interface ConfigNetworkView {
  label: string;
  masked: string;
}

/** Own port publication in the config; `for` — `"all"` or a list of full ids. */
export interface PublishedPortConfig {
  port: number;
  name: string;
  for: "all" | string[];
}

/** Another member's port opened here: `node` — full id, `local` — the listener port. */
export interface OpenedPortConfig {
  node: string;
  port: number;
  local: number;
}

/** `autostart`/`full_tunnel`: `available` and `reason` are inserted by the node, they are not in the file. */
export interface UnavailableFeature {
  enabled: boolean;
  available: boolean;
  reason: string;
}

/** Dotted path of a config field: `"socks.port"`, `"route.exits"`… */
export type ConfigPath = string;

/** Error of a single field: in `file.field_errors`, in `fields` of the 422 responses. */
export interface FieldError {
  path: ConfigPath;
  message: string;
}

/** The `file` block — the state of the settings file (`ConfigState::file_block`) — WEB-API-config.md §1.6. */
export interface ConfigFileInfo {
  path: string | null; // null — the node runs without a file (--no-config)
  state: ConfigFileState;
  writable: boolean;
  message: string | null;
  field_errors: FieldError[];
}

/** `GET /api/config` — WEB-API-config.md §1.6. */
export interface ConfigResponse {
  rev: number;
  config: ConfigDocument;
  read_only: ConfigPath[];
  restart_fields: ConfigPath[];
  restart_pending: ConfigPath[];
  cli_overrides: ConfigPath[];
  file: ConfigFileInfo;
}

/** Successful `PUT /api/config`, 200 — WEB-API-config.md §1.7. */
export interface ConfigPutResponse extends ConfigResponse {
  changed: ConfigPath[];
}

/** Partial object: nested objects are partial too, lists are whole. */
export type DeepPartial<T> = T extends readonly unknown[]
  ? T
  : T extends object
    ? { [K in keyof T]?: DeepPartial<T[K]> }
    : T;

/** Body of `PUT /api/config` — WEB-API-config.md §1.7. The node overlays `config` on the current
 * document (objects merge, lists are replaced); it ignores `read_only` fields. */
export interface ConfigPutRequest {
  rev: number;
  config: DeepPartial<ConfigDocument>;
}

/** Config meta lists in the page store (`store.configMeta`, `core/save-config.js`). */
export type ConfigMeta = Pick<ConfigResponse, "read_only" | "restart_fields" | "restart_pending" | "cli_overrides">;

/** Backup — WEB-API-config.md §1.9: not implemented in the node, no addresses; the type follows the prose. */
export type ConfigExportResponse = ConfigResponse; // GET /api/config/export — not confirmed by the code
export interface ConfigImportRequest {
  // POST /api/config/import — not confirmed by the code
  rev: number | "force";
  config: DeepPartial<ConfigDocument>;
}
