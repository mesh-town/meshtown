// The newest known release: the latest GitHub release (asked on mount and every 6 hours) or a newer version
// announced by a member, whichever is higher. A failed request leaves the GitHub part empty and nothing else changes.
import { computed, onMounted, onUnmounted, ref, type Ref } from "vue";
import type { NodeState } from "../core/api-types-state.ts";
import { UPDATE_LATEST_API_URL } from "./update-texts.ts";
import { compareVersions, newestVersion, parseVersion } from "./update-version.ts";

const RECHECK_MS = 6 * 60 * 60 * 1000;
const DISMISS_PREFIX = "update-dismissed-";

async function fetchLatestTag(): Promise<string | null> {
  try {
    const res = await fetch(UPDATE_LATEST_API_URL, { headers: { Accept: "application/vnd.github+json" } });
    const body = res.ok ? await res.json() : null;
    return typeof body?.tag_name === "string" ? body.tag_name : null;
  } catch {
    return null;
  }
}

function readDismissed(version: string): boolean {
  try {
    return localStorage.getItem(DISMISS_PREFIX + version) === "1";
  } catch {
    return false;
  }
}

function writeDismissed(version: string): void {
  try {
    localStorage.setItem(DISMISS_PREFIX + version, "1");
  } catch {
    // Storage unavailable: the banner just comes back on the next visit.
  }
}

export function useUpdateCheck(state: Ref<NodeState | null>) {
  const fromGithub = ref<string | null>(null);
  const dismissedNow = ref<string | null>(null);
  const own = computed(() => state.value?.node.version ?? null);
  const latest = computed(() => newestVersion([fromGithub.value, ...(state.value?.peers ?? []).map((p) => p.version)]));
  const newer = computed(() => (latest.value !== null && compareVersions(latest.value, own.value) === 1 ? latest.value : null));
  const visible = computed(() => newer.value !== null && dismissedNow.value !== newer.value && !readDismissed(newer.value));

  async function refresh(): Promise<void> {
    const tag = await fetchLatestTag();
    fromGithub.value = parseVersion(tag) === null ? null : tag;
  }

  function dismiss(): void {
    if (newer.value === null) return;
    writeDismissed(newer.value);
    dismissedNow.value = newer.value;
  }

  let timer: ReturnType<typeof setInterval> | undefined;
  onMounted(() => {
    void refresh();
    timer = setInterval(() => void refresh(), RECHECK_MS);
  });
  onUnmounted(() => clearInterval(timer));
  return { own, newer, visible, dismiss };
}
