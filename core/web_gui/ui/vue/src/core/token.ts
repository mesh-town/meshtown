/* The access token in sessionStorage. The token from the address is put into storage by `shell/token-url.js::adoptUrlToken`,
 * the new one after "Revoke" by `setToken`; the key is shared. */

export const TOKEN_KEY = "meshtown.token";

type TokenStorage = Pick<Storage, "getItem" | "setItem">;

export function getToken(storage: TokenStorage = sessionStorage): string {
  try {
    return storage.getItem(TOKEN_KEY) || "";
  } catch (err) {
    return "";
  }
}

export function setToken(token: string, storage: TokenStorage = sessionStorage): void {
  try {
    storage.setItem(TOKEN_KEY, token);
  } catch (err) {
    // storage unavailable — the token stays as it was
  }
}
