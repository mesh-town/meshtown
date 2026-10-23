import type { Page } from '@playwright/test';

/** The primary network slot id in the overview. */
const MAIN_SLOT = 'main';

interface OverviewSlot {
  slot?: string;
  state?: unknown;
}

interface OverviewDoc {
  slots?: OverviewSlot[];
}

/**
 * Rewrites the primary network state inside the `GET /api/overview` reply the page polls:
 * the real reply of the target, with `slots[main].state` replaced by `patch(state)`.
 */
export async function patchMainState<T extends object = Record<string, unknown>>(page: Page, patch: (state: T) => T): Promise<void> {
  await page.route('**/api/overview', async (route) => {
    const res = await route.fetch();
    const body = (await res.json()) as OverviewDoc;
    const main = (body.slots ?? []).find((s) => s.slot === MAIN_SLOT);
    if (main && main.state) main.state = patch(main.state as T);
    await route.fulfill({ response: res, json: body });
  });
}
