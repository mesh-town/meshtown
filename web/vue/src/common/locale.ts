/* Locale helpers: the page language lives in one place. */

// The locale used for number formatting and sorting across the page.
export const LOCALE = "en-US";

// Picks the singular form for exactly 1 and the plural form for everything else (including 0).
export function plural(n: number, one: string, other: string): string {
  return n === 1 ? one : other;
}

// Formats a number with the locale's digit grouping (12345 -> "12,345").
export function formatNumber(n: number): string {
  return n.toLocaleString(LOCALE);
}

// Compares two labels alphabetically for the locale (not by character code); usable as a sort callback.
export function compareLabels(a: string, b: string): number {
  return a.localeCompare(b, LOCALE);
}
