/* What to show in the "Settings" section: the field row rules, notes, the file banner, the
 * save bar. No markup and no Vue. */

import type { ConfigDocument, ConfigFileInfo, ConfigMeta, FieldError, NodeState } from "../core/api-types.ts";
import { forDevice } from "../common/device.ts";
import { CLI_FIXED_NOTE, upperFirst } from "../common/words.ts";
import type { Store } from "../core/store.ts";
import type { ConfigChanges } from "../core/save-config.ts";
import { EXPERT_FIELDS, PAGE_FIELDS, SHARED_PATHS, controlValue, fid, getPath } from "./settings-fields.ts";
import type { Field, RawValue } from "./settings-fields.ts";
import {
  SETTINGS_ALL_NETS, SETTINGS_WEB_ACCESS_MULTI, SETTINGS_BROKEN_FILE, SETTINGS_CLI_NOTE_EDITABLE, SETTINGS_CLI_TAIL, SETTINGS_READONLY_DEFAULT, SETTINGS_READONLY_WEB_ENABLED,
  SETTINGS_RESTART_STARTED, SETTINGS_SAVED, SETTINGS_SAVED_LATE, SETTINGS_VALUE_OFF, SETTINGS_VALUE_ON, SETTINGS_WRITABLE_REASON,
  settingsCliHeadText,
} from "./settings-texts.ts";

type StateLike = Partial<NodeState> | null | undefined;
type MetaLike = ConfigMeta | null | undefined;

/** Everything needed for the view of a single field row. */
interface FieldContext {
  config: ConfigDocument | null;
  meta: MetaLike;
  state: StateLike;
  draft?: Record<string, RawValue>;
  errors?: Record<string, string>;
}

export interface FieldView {
  id: string;
  value: RawValue;
  readOnly: boolean;
  disabled: boolean;
  editable: boolean;
  restartBadge: boolean;
  readonlyNote: string;
  cliNote: string;
  disabledReason: string;
  error: string;
}

export interface FileBanner {
  broken: { message: string } | null;
  fieldErrors: FieldError[];
}

export interface MessageView {
  text: string;
  cls: string;
}

interface SaveBarInput {
  dirty: boolean;
  meta: MetaLike;
  configFile: ConfigFileInfo | null | undefined;
  requested: boolean;
  busy: boolean;
  message: MessageView | null | undefined;
  restartTargets?: string[]; // slots waiting for a restart; omitted — derived from `meta`
}

export interface SaveBarView {
  visible: boolean;
  cls: string;
  dirtyHint: boolean;
  saveDisabled: boolean;
  writableReason: string;
  restartOffered: boolean;
  messageShown: string;
}

export const RESTART_STARTED = SETTINGS_RESTART_STARTED;

// Whether the node has fields waiting for a restart.
export function restartPending(meta: MetaLike): boolean {
  return !!(meta && meta.restart_pending && meta.restart_pending.length > 0);
}

// Slots to restart: the ones the overview lists, else the main one when the config says a restart is pending (not while the overview is the source).
export function restartTargets(store: Pick<Store, "restartSlots" | "configMeta" | "source" | "state">): string[] {
  if (store.restartSlots.length > 0) return store.restartSlots;
  // A failed start leaves nothing pending, yet a restart is the way out.
  if (store.state?.node?.start_error) return ["main"];
  // The overview already said nothing waits; the config of the main node is read later and may still be stale.
  if (store.source === "overview") return [];
  return restartPending(store.configMeta) ? ["main"] : [];
}

// The part of the changes that every network keeps in step.
export function sharedChanges(changes: ConfigChanges): ConfigChanges {
  return Object.fromEntries(Object.entries(changes).filter(([path]) => SHARED_PATHS.includes(path)));
}

const ALL_NETS_FIELDS: readonly string[] = [...EXPERT_FIELDS, ...PAGE_FIELDS].map((f) => f.path);

// A note under the rows that apply to every network.
export function fieldCaption(field: Field, multi: boolean): string {
  return multi && ALL_NETS_FIELDS.includes(field.path) ? SETTINGS_ALL_NETS : "";
}

export function fieldLabel(field: Field, multi: boolean): string {
  return multi && field.path === "web.access" ? forDevice(SETTINGS_WEB_ACCESS_MULTI) : field.title;
}

export function metaHas(meta: MetaLike, key: keyof ConfigMeta, path: string): boolean {
  if (!meta) return false;
  return (meta[key] || []).includes(path);
}

function portOf(address: unknown): number | undefined {
  if (typeof address !== "string") return undefined;
  const m = /:(\d+)$/.exec(address);
  return m ? parseInt(m[1], 10) : undefined;
}

// The effective value of a flag-overridden field comes from the node snapshot: the config document carries the file one.
const CLI_OVERRIDE_EFFECTIVE: Record<string, (state: StateLike) => number | undefined> = {
  "socks.port": (state: StateLike) => portOf(state && state.socks && state.socks.listen),
  "web.port": (state: StateLike) => portOf(state && state.node && state.node.web_addr),
};

function effectiveValue(path: string, state: StateLike): number | undefined {
  const read = CLI_OVERRIDE_EFFECTIVE[path];
  if (!read) return undefined;
  const v = read(state);
  return v === null ? undefined : v;
}

function noteValue(v: unknown): string {
  if (v === null || v === undefined || v === "") return "—";
  if (typeof v === "boolean") return v ? SETTINGS_VALUE_ON : SETTINGS_VALUE_OFF;
  return String(v);
}

export function cliNoteText(path: string, readOnly: boolean, state: StateLike, config: ConfigDocument | null): string {
  const eff = effectiveValue(path, state);
  if (eff === undefined) {
    return readOnly ? CLI_FIXED_NOTE : SETTINGS_CLI_NOTE_EDITABLE;
  }
  const head = settingsCliHeadText(noteValue(getPath(config, path)), noteValue(eff));
  return readOnly ? head : head + SETTINGS_CLI_TAIL;
}

function readonlyNoteText(path: string): string {
  if (path === "web.enabled") return SETTINGS_READONLY_WEB_ENABLED;
  return SETTINGS_READONLY_DEFAULT;
}

// Read-only shows the config value as is (a boolean — a locked switch,
// otherwise — text); an editable field shows the draft, else the config value.
function shownValue(field: Field, readOnly: boolean, configValue: unknown, draft: Record<string, RawValue>): RawValue {
  if (readOnly) return typeof configValue === "boolean" ? configValue : controlValue(configValue, "text") || "—";
  if (field.path in draft) return draft[field.path];
  return controlValue(configValue, field.kind);
}

export function fieldView(field: Field, ctx: FieldContext): FieldView {
  const path = field.path;
  const readOnly = field.kind === "readonly" || metaHas(ctx.meta, "read_only", path);
  const cli = metaHas(ctx.meta, "cli_overrides", path);
  const lock = field.disabledBy ? field.disabledBy(ctx.config) : { disabled: false, reason: "" };
  const forced = !readOnly && lock.disabled;
  return {
    id: fid(path),
    value: shownValue(field, readOnly, getPath(ctx.config, path), ctx.draft || {}),
    readOnly,
    disabled: readOnly || lock.disabled,
    editable: !readOnly,
    restartBadge: metaHas(ctx.meta, "restart_fields", path) || metaHas(ctx.meta, "restart_pending", path),
    readonlyNote: readOnly && !cli ? readonlyNoteText(path) : "",
    cliNote: cli ? cliNoteText(path, readOnly, ctx.state, ctx.config) : "",
    disabledReason: forced && !cli && lock.reason ? lock.reason : "",
    error: (ctx.errors && ctx.errors[path]) || "",
  };
}

export function savedMessageText(meta: MetaLike, paths: string[]): string {
  const late = paths.some((p) => metaHas(meta, "restart_fields", p) || metaHas(meta, "restart_pending", p));
  return late ? SETTINGS_SAVED_LATE : SETTINGS_SAVED;
}

// `null` if there is nothing to show.
export function fileBanner(configFile: ConfigFileInfo | null | undefined, hideFieldErrors: boolean): FileBanner | null {
  if (!configFile) return null;
  const invalid = configFile.state === "invalid" || configFile.state === "unreadable";
  const broken = invalid ? { message: configFile.message || SETTINGS_BROKEN_FILE } : null;
  const listed = !hideFieldErrors && configFile.field_errors ? configFile.field_errors : [];
  if (!broken && listed.length === 0) return null;
  return { broken, fieldErrors: listed };
}

function barClass(dirty: boolean, pending: boolean): string {
  if (dirty) return "save-bar dirty";
  return pending ? "save-bar saved-sticky" : "save-bar";
}

export function saveBarView({ dirty: isDirty, meta, configFile, requested, busy, message, restartTargets: targets }: SaveBarInput): SaveBarView {
  const dirty = !!isDirty;
  const writable = !configFile || configFile.writable !== false;
  const slots = targets ?? (restartPending(meta) ? ["main"] : []);
  const pending = restartPending(meta) || slots.length > 0;
  const text = (message && message.text) || "";
  return {
    visible: dirty || !writable || pending,
    cls: barClass(dirty, pending),
    dirtyHint: dirty,
    saveDisabled: !writable || !!busy,
    writableReason: writable ? "" : (upperFirst(configFile && configFile.message) || SETTINGS_WRITABLE_REASON),
    restartOffered: slots.length > 0 && !requested,
    messageShown: text === RESTART_STARTED && !pending ? "" : text,
  };
}
