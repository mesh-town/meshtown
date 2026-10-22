/* Which screen to show: the shell, sign-in or the first password. The node status (`GET /api/auth/status`) is read
 * on mount; a 401 on any poller request (`conn.awaitingToken`) also leads to sign-in. If the status is not
 * received (old node, no network) — the shell, as before. */

import { computed, onMounted, ref } from "vue";
import type { ComputedRef, Ref } from "vue";
import { getToken } from "../core/token.ts";
import type { Conn } from "../shell/conn.ts";
import { authStatus } from "./login-api.ts";
import type { AuthStatus } from "./login-api.ts";

export type Screen = "app" | "login" | "setup";

export interface AuthDeps {
  authStatus: () => Promise<AuthStatus>;
  getToken: () => string;
}

export interface Auth {
  screen: ComputedRef<Screen>;
  hasSession: ComputedRef<boolean>;
  noPassword: ComputedRef<boolean>;
  loggedIn: () => void;
  passwordSet: () => void;
}

const defaultAuthDeps: AuthDeps = { authStatus, getToken: () => getToken() };

// No session and no token to get one — sign-in only.
function needsLogin(status: AuthStatus | null, token: string): boolean {
  return status !== null && status.via === null && !token;
}

// Only a session from a temporary token, and only while there is no password.
function needsSetup(status: AuthStatus | null, passwordDone: boolean): boolean {
  return !passwordDone && status !== null && status.via === "temp" && !status.password_set;
}

function pick(status: Ref<AuthStatus | null>, conn: Conn, deps: AuthDeps, passwordDone: boolean): Screen {
  if (needsSetup(status.value, passwordDone)) return "setup";
  if (conn.awaitingToken) return "login";
  return needsLogin(status.value, deps.getToken()) ? "login" : "app";
}

export function useAuth(conn: Conn, deps: AuthDeps = defaultAuthDeps): Auth {
  const status = ref<AuthStatus | null>(null);
  const passwordDone = ref(false);
  const screen = computed(() => pick(status, conn, deps, passwordDone.value));
  const hasSession = computed(() => status.value !== null && status.value.via !== null);
  const noPassword = computed(() => status.value !== null && status.value.password_set === false);
  onMounted(async () => {
    status.value = await deps.authStatus().catch(() => null);
  });
  const loggedIn = () => {
    status.value = { password_set: true, access: "localnet", listening: "localnet", ...status.value, via: "password" };
  };
  return { screen, hasSession, noPassword, loggedIn, passwordSet: () => { passwordDone.value = true; } };
}
