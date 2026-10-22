/* Token from the address before mounting: exchange for a session (`POST /api/auth/token`). A temporary token is not put in
 * `sessionStorage`; a permanent one stays for the header. A failed exchange is not fatal: an old
 * node or a stub answers 404 — the page works by the header, as before. */

import { setToken } from "../core/token.ts";
import { takeTokenFromUrl } from "../shell/token-url.ts";
import type { ShellDeps } from "../shell/shell-deps.ts";
import { exchangeToken } from "./login-api.ts";

type ExchangeDeps = Pick<ShellDeps, "href" | "storage" | "replaceUrl">;

// Find out what the token was; `null` — the exchange failed and there is nothing to say.
async function viaOf(token: string): Promise<"temp" | "token" | null> {
  const res = await exchangeToken(token).catch(() => null);
  return res ? res.via : null;
}

export async function adoptAndExchangeToken(deps: ExchangeDeps): Promise<void> {
  const found = takeTokenFromUrl(deps.href);
  if (!found) return;
  deps.replaceUrl(found.cleanUrl);
  const via = await viaOf(found.token);
  if (via !== "temp") setToken(found.token, deps.storage);
}
