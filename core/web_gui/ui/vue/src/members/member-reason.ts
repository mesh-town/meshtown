/* The optional reason a member gives when proposing a ban; the other members see it. */

export const REASON_MAX = 80;

// Control characters become spaces, the ends are trimmed, the text is cut to `REASON_MAX` code points.
export function cleanReason(text: string): string {
  const spaced = text.replace(/[\u0000-\u001f\u007f-\u009f]/g, " ").trim();
  return Array.from(spaced).slice(0, REASON_MAX).join("").trim();
}
