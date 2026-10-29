/* Texts of the "Send exit traffic through" field of the device block. */

import { forDevice } from "../common/device.ts";

export const EXIT_IFACE_LABEL = "Send exit traffic through";
export const EXIT_IFACE_ANY = "Any network (system routing)";
export const EXIT_IFACE_NOT_FOUND_SUFFIX = " (not found)";
export const exitIfaceHint = (): string =>
  forDevice("Pick a VPN interface to send sites opened through this computer only via that VPN. If it disconnects, those sites stop opening instead of using your regular network.");

export const exitIfaceMissing = (name: string): string => forDevice(`${name} is not on this computer — exit connections are refused`);
export const exitIfaceDown = (name: string): string => `${name} is down — exit connections are refused`;
export const exitIfaceUnsupported = (): string => "This system can't bind exit traffic to an interface — exit connections are refused";
