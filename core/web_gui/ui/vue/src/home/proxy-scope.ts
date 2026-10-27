// Where the proxy is meant to work: in the whole system or only in the browser. Remembered in the page storage
// next to the setup mark; storage errors are swallowed.

export type ProxyScope = "system" | "browser";

interface ScopeStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

const SCOPE_KEY = "meshtown.proxyScope";

// Anything but the two known values (including no storage) is "not chosen yet".
export function readProxyScope(storage: ScopeStorage | null): ProxyScope | null {
  try {
    const raw = storage!.getItem(SCOPE_KEY);
    return raw === "system" || raw === "browser" ? raw : null;
  } catch (err) {
    return null;
  }
}

export function writeProxyScope(scope: ProxyScope, storage: ScopeStorage | null): void {
  try {
    storage!.setItem(SCOPE_KEY, scope);
  } catch (err) {
    /* storage is unavailable — the choice is asked again next time */
  }
}
