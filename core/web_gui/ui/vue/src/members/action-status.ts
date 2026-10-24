/* The line under the member list: the two looks of it and the text of a refused action. */

import type { ExitSaveStatus } from "../core/exit-via.ts";
import { memberErrorText } from "./members-api.ts";

export const INFO = (text: string): ExitSaveStatus => ({ text, cls: "" });
export const FAILED = (text: string): ExitSaveStatus => ({ text, cls: "field-error-text" });

// The node writes its own sentence for a refused action; without one the text comes from the error code.
export function failureText(err: unknown): string {
  const said = (err as { body?: { message?: unknown } } | null)?.body?.message;
  return typeof said === "string" && said.trim() ? said : memberErrorText(err);
}

export const errorCode = (err: unknown): string => {
  const code = (err as { code?: unknown } | null)?.code;
  return typeof code === "string" ? code : "";
};
