/* Writes text to the clipboard through `deps.clipboard` (shared helper for "Home" and "Diagnostics").
 * Returns true when the text went to the clipboard; with no clipboard or a refusal it returns false and throws nothing. */

export interface ClipboardDeps {
  clipboard?: { writeText: (text: string) => Promise<void> } | null;
}

export async function writeClipboard(deps: ClipboardDeps, text: string): Promise<boolean> {
  const clipboard = deps.clipboard;
  if (!clipboard) return false;
  try {
    await clipboard.writeText(text);
    return true;
  } catch (err) {
    return false;
  }
}
