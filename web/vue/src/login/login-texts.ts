// Sign-in texts: the password screen, the first password setup, the "password not set yet" instructions.

import { NET_NAME } from "../common/words.ts";

export const LOGIN_TOO_MANY_TRIES = "Too many attempts. Try again a bit later.";

export function loginTooManyTriesInText(sec: number): string {
  return "Too many attempts. Try again in " + sec + " s.";
}

export const LOGIN_WRONG_PASSWORD = "Wrong password";
export const LOGIN_FAILED = "Couldn't sign in";

export function loginHttpErrorText(status: number): string {
  return NET_NAME + " responded with an error (code " + status + ")";
}

export const NOPW_INTRO = "To set a password, you need a one-time link. " + NET_NAME + " prints it when you run the command below.";
export const NOPW_ROUTER_NOTE = "On an OpenWrt router, " + NET_NAME + " runs as a service, so the command is:";
export const NOPW_STEP_LINK = "The command prints a sign-in link — open it in this browser. The link is valid for 10 minutes and works once.";
export const NOPW_STEP_SET = "The link opens a window where you create a password; after that you sign in with it.";
export const NOPW_RUN_LOCAL = "On this computer, run in a terminal:";
export const NOPW_COPY = "Copy";

export function nopwRunRemoteText(host: string): string {
  return "On " + host + " (the computer running " + NET_NAME + "), run this in a terminal:";
}

export const LOGIN_NO_PASSWORD_TITLE = "Password not set yet";
export const LOGIN_TITLE = "Sign in";
export const LOGIN_HINT = "This page is password-protected. Enter the password you set the first time you opened it.";
export const LOGIN_PASSWORD_LABEL = "Password";
export const LOGIN_SUBMIT = "Sign in";

export const SETUP_NO_STORE = "SetupScreen: store not provided (STORE_KEY)";
export const SETUP_TITLE = "Set a password";
export const SETUP_HINT = "The password protects this settings page. From now on, you'll sign in with it instead of the link " + NET_NAME + " printed.";
export const SETUP_AGAIN_LABEL = "Password again";
export const SETUP_MISMATCH = "Passwords don't match";
export const SETUP_ACCESS_LABEL = "Who can access the settings page";
export const SETUP_ACCESS_HINT = "You can change this later in \"Settings\". Note: this only concerns access to the node settings, not to the network or traffic.";
export const SETUP_CONTINUE = "Continue";
export const LOGIN_SET_PASSWORD = "Set password";

export function setupPasswordLabelText(minLength: number): string {
  return "Password (at least " + minLength + " characters)";
}

export function setupPasswordFailedText(reason: string): string {
  return "Couldn't set the password: " + reason;
}

export function setupAccessFailedText(reason: string): string {
  return "Password set, but access wasn't saved: " + reason + ". Click \"Continue\" again.";
}

export function passwordSaveFailedText(reason: string): string {
  return "Couldn't save the password: " + reason;
}

export const PASSWORD_SAVED = "Password saved. Other open sessions were closed.";

export function passwordLogoutFailedText(reason: string): string {
  return "Couldn't sign out: " + reason;
}

export const PASSWORD_ROW_TITLE = "Page password";
export const PASSWORD_IS_SET = "A password is set. Use it to sign in to this page.";
export const PASSWORD_NOT_SET = "No password yet. The page opens only from the link " + NET_NAME + " prints at startup.";
export const PASSWORD_NEEDS_RESTART = "The chosen page access will take effect after the node restarts.";
export const PASSWORD_NEW_LABEL = "New password";
export const PASSWORD_CHANGE = "Change password";
export const PASSWORD_LOGOUT = "Sign out";
