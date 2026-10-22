import { test, expect } from '../fixtures/index';

// The password hash in a debug build takes up to ~1.5 s; sign-in and change have a margin.
const HASH_TIMEOUT_MS = 15000;
const PASSWORD = 'e2e-password-4821';

test('73-auth-login: permanent token exchange, "Set password", "Sign out", password sign-in and a wrong password; at the end reset returns the node to no password', async ({
  app,
  page,
  target,
  targetKind,
}) => {
  test.skip(targetKind !== 'node', 'the page password lives on a real node: the demo node has no sign-in checks');
  test.setTimeout(90000);
  const shell = page.locator('nav#tabs');
  const loginForm = page.locator('#login-form');

  try {
    await test.step('exchanging the permanent token from the address gives a working page and a session cookie', async () => {
      await expect(shell).toBeVisible();
      await expect(loginForm).toHaveCount(0);
      const cookies = await page.context().cookies(target.origin);
      expect(cookies.length, 'session cookie after the exchange').toBeGreaterThan(0);
    });

    await test.step('"Settings" → "Set password": a saved message, the button becomes "Change password"', async () => {
      await app.tabs.open('settings');
      await app.settings.openExpert();
      await page.getByRole('textbox', { name: 'New password' }).fill(PASSWORD);
      await page.getByRole('button', { name: 'Set password', exact: true }).click();
      await expect(page.locator('#password-message')).toContainText('Password saved', { timeout: HASH_TIMEOUT_MS });
      await expect(page.getByRole('button', { name: 'Change password', exact: true })).toBeVisible();
    });

    await test.step('"Sign out" → sign-in screen', async () => {
      await page.getByRole('button', { name: 'Sign out', exact: true }).click();
      await expect(loginForm).toBeVisible();
      await expect(shell).toHaveCount(0);
    });

    await test.step('wrong password — an error message, the sign-in screen stays', async () => {
      await page.getByLabel('Password', { exact: true }).fill('not-the-right-password-at-all');
      await page.getByRole('button', { name: 'Sign in', exact: true }).click();
      await expect(page.getByRole('alert')).toBeVisible({ timeout: HASH_TIMEOUT_MS });
      await expect(loginForm).toBeVisible();
    });

    await test.step('correct password → page shell', async () => {
      await page.getByLabel('Password', { exact: true }).fill(PASSWORD);
      await page.getByRole('button', { name: 'Sign in', exact: true }).click();
      await expect(shell).toBeVisible({ timeout: HASH_TIMEOUT_MS });
      await expect(loginForm).toHaveCount(0);
    });
  } finally {
    // The target is shared per worker: the password must not leak to neighboring specs.
    const reset = await page.request.post(`${target.origin}/api/auth/reset`, {
      headers: { 'X-Meshtown-Token': target.token, Origin: target.origin },
      failOnStatusCode: false,
    });
    expect(reset.status(), 'reset with the permanent token').toBe(200);
  }
});
