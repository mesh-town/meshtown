// Hover/focus explanations for the two technical words that appear in setup instructions.
import { NET_NAME } from "../../common/words.ts";

export const TERM_TIPS: Record<string, string> = {
  PAC: `A small file at this address tells programs which sites to open through ${NET_NAME} and which to open directly.`,
  SOCKS5: `A common proxy type. Programs that support it send their connections through ${NET_NAME}.`,
};

export interface TermPart { text: string; tip?: string }

/** Splits text into plain pieces and whole-word terms (exact case); joined pieces equal the input. */
export function termParts(text: string): TermPart[] {
  const parts: TermPart[] = [];
  let last = 0;
  for (const m of text.matchAll(/(?<![A-Za-z0-9])(PAC|SOCKS5)(?![A-Za-z0-9])/g)) {
    if (m.index > last) parts.push({ text: text.slice(last, m.index) });
    parts.push({ text: m[0], tip: TERM_TIPS[m[0]] });
    last = m.index + m[0].length;
  }
  if (last < text.length) parts.push({ text: text.slice(last) });
  return parts;
}
