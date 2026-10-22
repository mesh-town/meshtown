/* Applying a snapshot to the store — for tests only: on the page the store is written by the poller and sections. */

const FIELDS = ["state", "config", "configRev", "configMeta", "configFile", "activeSection"];

export function applySnapshot(store, detail) {
  if (!detail || typeof detail !== "object") return;
  for (const key of FIELDS) {
    if (Object.hasOwn(detail, key)) store[key] = detail[key];
  }
}
