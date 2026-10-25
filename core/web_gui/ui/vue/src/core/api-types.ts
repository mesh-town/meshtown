/* Types of the node HTTP API responses and requests — handwritten, types only, no runtime code.
 * Source of truth — the handlers in `core/web_gui/` (serde structs in `snapshot.rs`, responses in
 * `routes/*.rs`, `config_api/*.rs`); prose — `docs/WEB-API.md`, `docs/WEB-API-config.md`.
 * `Option<T>` in Rust is `T | null` here (the field is always present); `?` — the field may be absent entirely. */

export type * from "./api-types-state.ts";
export type * from "./api-types-config.ts";
export type * from "./api-types-actions.ts";
export type * from "./api-types-overview.ts";
