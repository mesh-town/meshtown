// Release version comparison for the "update available" banner: numeric, part by part (`X.Y.Z`).

/** Parses `1.2.3` (an optional leading `v`) into numbers; `null` for anything that is not dotted digits. */
export function parseVersion(raw: string | null | undefined): number[] | null {
  const text = (raw ?? "").trim().replace(/^v/, "");
  return /^\d+(\.\d+)*$/.test(text) ? text.split(".").map(Number) : null;
}

/** `1` when `a` is newer than `b`, `-1` when older, `0` when equal; `null` when either is not a version. */
export function compareVersions(a: string | null | undefined, b: string | null | undefined): number | null {
  const pa = parseVersion(a);
  const pb = parseVersion(b);
  return pa && pb ? comparePartwise(pa, pb) : null;
}

function comparePartwise(a: number[], b: number[]): number {
  const len = Math.max(a.length, b.length);
  const diff = Array.from({ length: len }, (_, i) => Math.sign((a[i] ?? 0) - (b[i] ?? 0))).find((d) => d !== 0);
  return diff ?? 0;
}

/** The newest of the given versions (garbage skipped); `null` when none is valid. */
export function newestVersion(candidates: Array<string | null | undefined>): string | null {
  const valid = candidates.filter((v): v is string => parseVersion(v) !== null).map((v) => v.trim().replace(/^v/, ""));
  return valid.reduce<string | null>((best, v) => (best === null || compareVersions(v, best) === 1 ? v : best), null);
}
