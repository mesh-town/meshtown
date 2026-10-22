/* An invite handed to the page in the address (`?invite=mesh-invite…`), e.g. by the Android app after "Share".
 * Pure functions: the field is filled in, never submitted, and the address is cleaned afterwards. */

// The invite from a query string; empty when absent or when the value is not an invite.
export function inviteFromSearch(search: string): string {
  const value = new URLSearchParams(search).get("invite");
  const text = value === null ? "" : value.trim();
  return text.startsWith("mesh-invite") ? text : "";
}

// The same address without the `invite` parameter.
export function withoutInvite(search: string, path: string, hash: string): string {
  const params = new URLSearchParams(search);
  params.delete("invite");
  const rest = params.toString();
  return path + (rest ? "?" + rest : "") + hash;
}
