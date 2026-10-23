/* The fields of the "Settings" section and the control values.
 * Pure functions without Vue: the draft stores the raw control value, parsing by `dataType` happens on save. */

import type { ConfigDocument, FieldError, UnavailableFeature } from "../core/api-types.ts";
import { DESC_SOCKS_ACCESS_NETS } from "../lan/nets-texts.ts";
import {
  SETTINGS_DESC_HOST_PORT, SETTINGS_DESC_LOG_RING, SETTINGS_DESC_PROBE_INTERVAL, SETTINGS_DESC_PROVIDER_DNS, SETTINGS_DESC_SOCKS_ACCESS,
  SETTINGS_DESC_SOCKS_ENABLED, SETTINGS_DESC_WEB_ACCESS, SETTINGS_EVERY_10_MIN, SETTINGS_EVERY_15_MIN, SETTINGS_EVERY_30_MIN,
  SETTINGS_EVERY_5_MIN, SETTINGS_EVERY_6_HOURS, SETTINGS_EVERY_HOUR, SETTINGS_EVERY_MINUTE,
  SETTINGS_PLACEHOLDER_NOT_SET, SETTINGS_SOCKS_LOCALHOST, SETTINGS_SOCKS_LOCALNET,
  SETTINGS_TITLE_AUTOSTART, SETTINGS_TITLE_CHECKS_ENABLED, SETTINGS_TITLE_CHECKS_INTERVAL, SETTINGS_TITLE_DOH_SERVER,
  SETTINGS_TITLE_FULL_TUNNEL, SETTINGS_TITLE_LOG_RING, SETTINGS_TITLE_PROBE_INTERVAL, SETTINGS_TITLE_PROVIDER_DNS,
  SETTINGS_TITLE_SOCKS_ACCESS, SETTINGS_TITLE_SOCKS_ENABLED, SETTINGS_TITLE_SOCKS_PORT, SETTINGS_TITLE_WEB_ACCESS,
  SETTINGS_TITLE_WEB_ENABLED, SETTINGS_TITLE_WEB_PORT, SETTINGS_WEB_LOCALHOST, SETTINGS_WEB_LOCALNET, settingsFileValueText,
} from "./settings-texts.ts";

export type FieldKind = "toggle" | "text" | "select" | "lines" | "readonly";
export type DataType = "int" | "int_or_null" | "str" | "str_or_null" | "lines";
/** The raw control value in the draft: a switch is a boolean, anything else is a string. */
export type RawValue = string | boolean;
/** The value that goes into the config after `parseRaw`. */
export type ConfigValue = string | number | boolean | null | string[];
export type SelectOption = readonly [string | number, string];

export interface FieldLock {
  disabled: boolean;
  reason: string | undefined;
}

export interface Field {
  path: string;
  kind: FieldKind;
  title: string;
  dataType?: DataType;
  desc?: string;
  placeholder?: string;
  options?: readonly SelectOption[];
  disabledBy?: (config: ConfigDocument | null | undefined) => FieldLock;
}

export const CHECKS_INTERVAL_OPTIONS: readonly SelectOption[] = [
  [300, SETTINGS_EVERY_5_MIN],
  [900, SETTINGS_EVERY_15_MIN],
  [1800, SETTINGS_EVERY_30_MIN],
  [3600, SETTINGS_EVERY_HOUR],
  [21600, SETTINGS_EVERY_6_HOURS],
];
export const PROBE_INTERVAL_OPTIONS: readonly SelectOption[] = [
  [60, SETTINGS_EVERY_MINUTE],
  [300, SETTINGS_EVERY_5_MIN],
  [600, SETTINGS_EVERY_10_MIN],
  [1800, SETTINGS_EVERY_30_MIN],
  [3600, SETTINGS_EVERY_HOUR],
];

export const SOCKS_ACCESS_OPTIONS: readonly SelectOption[] = [
  ["localhost", SETTINGS_SOCKS_LOCALHOST],
  ["localnet", SETTINGS_SOCKS_LOCALNET],
];

export const WEB_ACCESS_OPTIONS: readonly SelectOption[] = [
  ["localhost", SETTINGS_WEB_LOCALHOST],
  ["localnet", SETTINGS_WEB_LOCALNET],
];

// The field is disabled while the platform can't do it: the reason comes from the config itself.
function availability(section: "autostart" | "full_tunnel"): (config: ConfigDocument | null | undefined) => FieldLock {
  return (config) => {
    const node: Partial<UnavailableFeature> = (config && config[section]) || {};
    return { disabled: !node.available, reason: node.reason };
  };
}

const DESC_SOCKS_ACCESS = SETTINGS_DESC_SOCKS_ACCESS;

/** Description of `socks.access`: with the networks block visible — about the networks checked below; otherwise the old one. */
export function socksAccessDesc(blockVisible: boolean): string {
  return blockVisible ? DESC_SOCKS_ACCESS_NETS : DESC_SOCKS_ACCESS;
}

export const MAIN_FIELDS: readonly Field[] = [
  { path: "autostart.enabled", kind: "toggle", title: SETTINGS_TITLE_AUTOSTART, disabledBy: availability("autostart") },
  { path: "socks.enabled", kind: "toggle", title: SETTINGS_TITLE_SOCKS_ENABLED, desc: SETTINGS_DESC_SOCKS_ENABLED },
  { path: "socks.port", kind: "text", dataType: "int", title: SETTINGS_TITLE_SOCKS_PORT, placeholder: "1080" },
  {
    path: "socks.access",
    kind: "select",
    dataType: "str",
    title: SETTINGS_TITLE_SOCKS_ACCESS,
    options: SOCKS_ACCESS_OPTIONS,
    desc: DESC_SOCKS_ACCESS,
  },
  { path: "checks.enabled", kind: "toggle", title: SETTINGS_TITLE_CHECKS_ENABLED },
  { path: "checks.interval_sec", kind: "select", dataType: "int", title: SETTINGS_TITLE_CHECKS_INTERVAL, options: CHECKS_INTERVAL_OPTIONS },
];

export const EXPERT_FIELDS: readonly Field[] = [
  { path: "checks.doh_server", kind: "text", dataType: "str", title: SETTINGS_TITLE_DOH_SERVER, desc: SETTINGS_DESC_HOST_PORT },
  {
    path: "checks.provider_dns",
    kind: "text",
    dataType: "str_or_null",
    title: SETTINGS_TITLE_PROVIDER_DNS,
    placeholder: SETTINGS_PLACEHOLDER_NOT_SET,
    desc: SETTINGS_DESC_PROVIDER_DNS,
  },
  {
    path: "probe.interval_sec",
    kind: "select",
    dataType: "int",
    title: SETTINGS_TITLE_PROBE_INTERVAL,
    options: PROBE_INTERVAL_OPTIONS,
    desc: SETTINGS_DESC_PROBE_INTERVAL,
  },
  {
    path: "log.ring_lines",
    kind: "text",
    dataType: "int",
    title: SETTINGS_TITLE_LOG_RING,
    desc: SETTINGS_DESC_LOG_RING,
  },
  { path: "full_tunnel.enabled", kind: "toggle", title: SETTINGS_TITLE_FULL_TUNNEL, disabledBy: availability("full_tunnel") },
];

export const PAGE_FIELDS: readonly Field[] = [
  { path: "web.enabled", kind: "readonly", title: SETTINGS_TITLE_WEB_ENABLED },
  { path: "web.port", kind: "readonly", title: SETTINGS_TITLE_WEB_PORT },
  {
    path: "web.access",
    kind: "select",
    dataType: "str",
    title: SETTINGS_TITLE_WEB_ACCESS,
    options: WEB_ACCESS_OPTIONS,
    desc: SETTINGS_DESC_WEB_ACCESS,
  },
];

// The same `id` as in the classic page: e2e relies on them.
export function fid(path: string): string {
  return "field-" + path.replace(/\./g, "-");
}

export function getPath(obj: unknown, path: string): unknown {
  let cur: unknown = obj;
  for (const key of path.split(".")) {
    if (cur === null || cur === undefined) return undefined;
    cur = (cur as Record<string, unknown>)[key];
  }
  return cur;
}

function parseInteger(raw: string): number | string | null {
  if (raw.trim() === "") return null;
  const n = parseInt(raw, 10);
  return isNaN(n) ? raw : n;
}

function parseLines(raw: string): string[] {
  return raw
    .split("\n")
    .map((s) => s.trim())
    .filter((s) => s.length);
}

const PARSERS: Partial<Record<DataType, (raw: string) => ConfigValue>> = {
  int: parseInteger,
  int_or_null: parseInteger,
  str_or_null: (raw: string) => (raw.trim() === "" ? null : raw),
  lines: parseLines,
};

// The raw control value → the value for the config.
export function parseRaw(raw: RawValue, dataType?: DataType): ConfigValue {
  if (typeof raw === "boolean") return raw;
  const parse = dataType && PARSERS[dataType];
  return parse ? parse(String(raw)) : String(raw);
}

// The config value → the control value.
export function controlValue(value: unknown, kind?: FieldKind): RawValue {
  if (kind === "toggle") return !!value;
  if (kind === "lines") return Array.isArray(value) ? value.join("\n") : "";
  if (value === null || value === undefined) return "";
  return String(value);
}

// The current value that is not in the list stays selectable (like `fieldRow`).
export function selectOptions(field: Field, value: unknown): readonly SelectOption[] {
  const options = field.options || [];
  if (options.some((pair) => String(pair[0]) === String(value))) return options;
  return [...options, [value as string | number, settingsFileValueText(value as string | number)] as const];
}

// Draft → config edits: a new object, `lines` get new arrays — proxies don't reach the core.
export function collectChanges(draft: Record<string, RawValue>, fields: readonly Field[]): Record<string, ConfigValue> {
  const changes: Record<string, ConfigValue> = {};
  for (const path of Object.keys(draft)) {
    const field = fields.find((f) => f.path === path);
    changes[path] = parseRaw(draft[path], field && field.dataType);
  }
  return changes;
}

// A field error line from the node: "{title} ({path}): {message}"; a field not in the lists — "{path}: {message}".
export function fieldErrorLine(fe: FieldError): string {
  const field = [...MAIN_FIELDS, ...EXPERT_FIELDS, ...PAGE_FIELDS].find((f) => f.path === fe.path);
  return field ? field.title + " (" + fe.path + "): " + fe.message : fe.path + ": " + fe.message;
}

/** Paths that every network keeps in step with the main one. */
export const SHARED_PATHS: readonly string[] = [
  "probe.interval_sec", "checks.enabled", "checks.interval_sec", "checks.doh_server", "checks.provider_dns",
];
