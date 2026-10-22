import { expect, type Page } from '@playwright/test';

const READY_TIMEOUT_MS = 10000;

interface StateBody {
  peers?: { id?: string; name?: string; self?: boolean }[];
}

const lastBodies = new WeakMap<Page, StateBody>();
const tracked = new WeakSet<Page>();

/** Listener of `/api/state` responses: the body of the last snapshot received by the page itself. */
export function trackState(page: Page): void {
  if (tracked.has(page)) return;
  tracked.add(page);
  page.on('response', async (res) => {
    if (res.request().method() !== 'GET' || new URL(res.url()).pathname !== '/api/state' || res.status() !== 200) return;
    try {
      lastBodies.set(page, (await res.json()) as StateBody);
    } catch {
      // the response was cut off by navigation — the next poll will bring a snapshot
    }
  });
}

/** Returns the body of the last `/api/state` response received by the page (empty until there was a response). */
export function lastState(page: Page): StateBody {
  return lastBodies.get(page) ?? {};
}

/** Page readiness: the shell set `data-ready` — the snapshot and the config revision are loaded. */
export async function waitReady(page: Page): Promise<void> {
  await expect(page.locator('[data-ready="true"]')).toBeAttached({ timeout: READY_TIMEOUT_MS });
}

/** Opens the address again (a full load) and waits for readiness. */
export async function gotoReady(page: Page, url: string): Promise<void> {
  trackState(page);
  await page.goto(url);
  await waitReady(page);
}
