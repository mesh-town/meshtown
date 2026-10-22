/* The token from the page address (a port of `02-token.js`): removed before the app is created,
 * so that the sections already see the token when mounted. */

import { setToken } from "../core/token.ts";
import type { ShellDeps } from "./shell-deps.ts";

// `?token=…` from the address → `{ token, cleanUrl }` (path + other search + hash); without a token — `null`.
export function takeTokenFromUrl(href: string): { token: string; cleanUrl: string } | null {
  const url = new URL(href);
  const token = url.searchParams.get("token");
  if (!token) return null;
  url.searchParams.delete("token");
  return { token, cleanUrl: url.pathname + url.search + url.hash };
}

// A pasted address (full or relative) → token; unparseable or without a token — `null`.
export function tokenFromPasted(text: unknown, baseHref: string): string | null {
  try {
    return new URL(String(text || "").trim(), baseHref).searchParams.get("token") || null;
  } catch (err) {
    return null;
  }
}

export function adoptUrlToken(deps: Pick<ShellDeps, "href" | "storage" | "replaceUrl">): void {
  const found = takeTokenFromUrl(deps.href);
  if (!found) return;
  setToken(found.token, deps.storage);
  deps.replaceUrl(found.cleanUrl);
}
