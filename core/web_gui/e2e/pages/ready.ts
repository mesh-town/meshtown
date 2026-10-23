import { expect, type Page } from '@playwright/test';

const READY_TIMEOUT_MS = 10000;

interface StateBody {
  peers?: { id?: string; name?: string; self?: boolean }[];
}

const lastBodies = new WeakMap<Page, StateBody>();
const tracked = new WeakSet<Page>();

/** The primary network state of an overview reply; the body itself for a plain `/api/state` reply. */
function mainState(path: string, body: unknown): StateBody {
  if (path === '/api/state') return body as StateBody;
  const slots = (body as { slots?: { slot?: string; state?: StateBody }[] }).slots ?? [];
  return slots.find((s) => s.slot === 'main')?.state ?? {};
}

/** Listener of `/api/overview` (and legacy `/api/state`) responses: the main-network state of the last snapshot received by the page itself. */
export function trackState(page: Page): void {
  if (tracked.has(page)) return;
  tracked.add(page);
  page.on('response', async (res) => {
    const path = new URL(res.url()).pathname;
    if (res.request().method() !== 'GET' || (path !== '/api/overview' && path !== '/api/state') || res.status() !== 200) return;
    try {
      lastBodies.set(page, mainState(path, await res.json()));
    } catch {
      // the response was cut off by navigation — the next poll will bring a snapshot
    }
  });
}

/** Returns the main-network state of the last snapshot received by the page (empty until there was a response). */
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
