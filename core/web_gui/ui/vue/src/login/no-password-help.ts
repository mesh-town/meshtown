/* Words of the "password not set yet" instructions on the sign-in screen. Pure functions: the host comes as a parameter
 * (`location.hostname`), no DOM needed. */

import { forDevice } from "../common/device.ts";
import { NOPW_INTRO, NOPW_ROUTER_NOTE, NOPW_RUN_LOCAL, NOPW_STEP_LINK, NOPW_STEP_SET, nopwRunRemoteText } from "./login-texts.ts";

export const CMD_RESET = "meshtown password reset";
export const CMD_ROUTER = "meshtown password reset --config /etc/meshtown/config.json";

const LOCAL_HOSTS = ["localhost", "127.0.0.1", "::1", "[::1]"];

export function isLocalHost(host: string): boolean {
  return LOCAL_HOSTS.includes(host);
}

export const INTRO = NOPW_INTRO;
export const ROUTER_NOTE = NOPW_ROUTER_NOTE;
export const STEP_LINK = NOPW_STEP_LINK;
export const STEP_SET = NOPW_STEP_SET;

export function stepRunText(host: string): string {
  if (isLocalHost(host)) return forDevice(NOPW_RUN_LOCAL);
  return nopwRunRemoteText(host);
}

export function showRouterLine(host: string): boolean {
  return !isLocalHost(host);
}
