import { test, expect } from '../fixtures/index';
import { GLOSSARY } from '../pages/glossary';
import type { Route } from '@playwright/test';

// Two poll cycles (2 s each) with a margin.
const POLL_CYCLES_MS = 6000;
const HELD_TIMEOUT_MS = 4000;

test('70-shell-rotate-token: "Revoke" in the same tab — a pending poll request may return 200 (the page holds a cookie session), a request without the cookie and with the old token gets 401, and there is no "The page was opened from an old or incomplete link" bar and the status line is alive', async ({
  app,
  page,
  request,
  target,
  targetKind,
}) => {
  test.skip(targetKind !== 'node', 'the demo target does not rotate the token: checked with a real node');
  let arm = false;
  let held: Route | null = null;
  let alive = 0;
  // The first poll request after `arm` does not reach the node until released: its token is the old one.
  await page.route('**/api/state', async (route) => {
    if (!arm || held) return route.continue();
    held = route;
  });

  await test.step('"Settings" → "Revoke" with a pending poll request — the new token is applied', async () => {
    await app.tabs.open('settings');
    await app.settings.openExpert();
    arm = true;
    await expect.poll(() => held !== null, { timeout: HELD_TIMEOUT_MS }).toBe(true);
    page.once('dialog', (dialog) => dialog.accept());
    const rotated = page.waitForResponse((r) => new URL(r.url()).pathname === '/api/token/rotate' && r.status() === 200);
    await page.getByRole('button', { name: 'Revoke', exact: true }).click();
    await rotated;
    await expect(page.getByText('The new token has been issued and applied')).toBeVisible();
  });

  await test.step('the pending request goes out with the old token and session (a 200 reply is fine); without the cookie the old token gives 401; after at least 2 poll cycles there is no bar', async () => {
    page.on('response', (r) => {
      if (new URL(r.url()).pathname === '/api/state' && r.status() === 200) alive++;
    });
    await held!.continue();
    // New model: rotate keeps the initiator session, so with the cookie the reply is 200; without the cookie the old token is rejected.
    const bare = await request.get(`${target.origin}/api/state`, { headers: { 'X-Meshtown-Token': target.token }, failOnStatusCode: false });
    expect(bare.status(), 'old token without cookie').toBe(401);
    await expect.poll(() => alive, { timeout: POLL_CYCLES_MS * 2, message: 'two /api/state polls with 200' }).toBeGreaterThanOrEqual(2);
    await expect(page.locator('#conn-strip')).toBeHidden();
    await expect(page.getByText(GLOSSARY.badTokenStrip)).toHaveCount(0);
    await expect(app.topPanel.statusLine).not.toHaveText(GLOSSARY.nodeNotResponding);
  });
});
