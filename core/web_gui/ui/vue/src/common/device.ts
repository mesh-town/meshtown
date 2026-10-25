/* Texts that name the device: inside the Android app the page runs on a phone, so "computer" reads "phone". */

import { inAppPage } from "../home/browser-setup.ts";

/** The text for the current device; call it inside a function, so the choice is made on every call. */
export function forDevice(text: string): string {
  return inAppPage() ? text.replace(/\bcomputer\b/g, "phone") : text;
}
