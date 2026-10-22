import { test, expect } from '../fixtures/index';

interface ConfigReply {
  rev?: number;
  config?: { checks?: { provider_dns?: string | null; doh_server?: string | null } };
}

const SAVED_TIMEOUT_MS = 6000;
const NEW_VALUE = '8.8.8.8:53';
const CONFLICT_VALUE = '1.1.1.1:53';
const FOREIGN_DOH = '9.9.9.9:9999';

test('11-save: PUT /api/config: a successful save is applied and shown on screen; a stale revision (409) does not silently erase what was typed', async ({
  app,
  api,
}) => {
  await app.tabs.open('settings');
  const settings = app.settings;

  const cfg0 = await api.getJson<ConfigReply>('/api/config');
  const rev0 = cfg0.body?.rev;
  const value0 = cfg0.body?.config?.checks?.provider_dns;

  await settings.saveProviderDns(NEW_VALUE);
  await expect(settings.savedMessage).toBeVisible({ timeout: SAVED_TIMEOUT_MS });

  await test.step('the save arrives and is applied', async () => {
    const cfg1 = await api.getJson<ConfigReply>('/api/config');
    const label = `rev ${rev0}→${cfg1.body?.rev}, checks.provider_dns ${value0}→${cfg1.body?.config?.checks?.provider_dns}`;
    expect.soft(cfg1.status, label).toBe(200);
    expect.soft(cfg1.body?.rev ?? -1, label).toBeGreaterThan(rev0 ?? Number.MAX_SAFE_INTEGER);
    expect.soft(cfg1.body?.config?.checks?.provider_dns, label).toBe(NEW_VALUE);
  });

  await test.step('the page shows success', async () => {
    await expect.soft(settings.savedMessage).toHaveClass(/saved/);
    await expect.soft(settings.savedMessage).toHaveText('Saved. Takes effect after Mesh Town restarts.');
  });

  // Scenario 2: an outside edit of checks.doh_server via a direct PUT while the page holds the old revision.
  const cfg2 = await api.getJson<ConfigReply>('/api/config');
  const revPage = cfg2.body?.rev;
  const dohDefault = cfg2.body?.config?.checks?.doh_server;
  const foreignBody = structuredClone(cfg2.body?.config ?? {}) as NonNullable<ConfigReply['config']>;
  foreignBody.checks = { ...foreignBody.checks, doh_server: FOREIGN_DOH };
  const foreign = await api.putJson('/api/config', { rev: revPage, config: foreignBody });
  test.skip(
    revPage === undefined || foreign.status !== 200,
    `not checked: the direct PUT that simulates an outside edit failed (status=${foreign.status})`,
  );

  await settings.saveProviderDns(CONFLICT_VALUE);

  await test.step('after the conflict the "changed in another tab" bar is shown', async () => {
    await expect.soft(settings.conflictHeading).toBeVisible({ timeout: SAVED_TIMEOUT_MS });
  });

  await test.step('next to the bar there is a button that reloads the form', async () => {
    await expect.soft(settings.conflictReload).toBeVisible();
  });

  await test.step('before the click: the field changed from outside is still old, and the user edit is intact', async () => {
    expect.soft(dohDefault, 'value of checks.doh_server before the conflict').toBeDefined();
    await expect.soft(settings.dohServer).toHaveValue(dohDefault ?? '\u0000');
    await expect.soft(settings.providerDns).toHaveValue(CONFLICT_VALUE);
  });

  await test.step('after the click: the form shows the value from the outside edit, saving again succeeds', async () => {
    await settings.conflictReload.click();
    await expect.soft(settings.dohServer).toHaveValue(FOREIGN_DOH, { timeout: SAVED_TIMEOUT_MS });
    await settings.saveProviderDns(CONFLICT_VALUE);
    await expect.soft(settings.savedMessage).toBeVisible({ timeout: SAVED_TIMEOUT_MS });
    await expect.soft(settings.savedMessage).toHaveClass(/saved/);
    const cfg3 = await api.getJson<ConfigReply>('/api/config');
    expect.soft(cfg3.status).toBe(200);
    expect.soft(cfg3.body?.rev ?? -1, 'rev grew by more than 1').toBeGreaterThan((revPage ?? Number.MAX_SAFE_INTEGER) + 1);
    expect.soft(cfg3.body?.config?.checks?.provider_dns).toBe(CONFLICT_VALUE);
  });
});

const RESTART_TIMEOUT_MS = 20000;
const RESTART_VALUE = '8.8.4.4:53';

// fast: 1 — one demo model second equals a real one, so `starting` (3 s) is caught by the page poll.
test.describe('restart button', () => {
  test.use({ targetFlags: { fast: 1 } });

test('52-save-restart-button: after saving an "after restart" field, a "Restart Mesh Town" button appears next to the message; clicking it restarts the node and clears the mark; a field without restart shows no button', async ({
  app,
  targetKind,
}) => {
  await app.tabs.open('settings');
  const settings = app.settings;

  await test.step('saving an "after restart" field: the message, the button and the sticky bar stay', async () => {
    await settings.saveProviderDns(RESTART_VALUE);
    await expect(settings.savedMessage).toHaveText('Saved. Takes effect after Mesh Town restarts.', {
      timeout: SAVED_TIMEOUT_MS,
    });
    await expect.soft(settings.restartButton).toBeVisible();
    await expect.soft(settings.saveBar).toHaveClass(/saved-sticky/);
    await expect.soft(settings.saveBar).toBeInViewport();
    // The message does not fade after 4 s while the restart has not been done.
    await settings.waitPastSavedTimer();
    await expect.soft(settings.savedMessage).toHaveText('Saved. Takes effect after Mesh Town restarts.');
    await expect.soft(settings.restartButton).toBeVisible();
  });

  await test.step('click: restart message, button hidden, then the bar and the message fade', async () => {
    await settings.restartButton.click();
    await expect.soft(settings.restartMessage).toBeVisible();
    await expect.soft(settings.restartButton).toBeHidden();
    await settings.expectStartingStatus(targetKind);
    await expect(settings.saveBar).not.toHaveClass(/saved-sticky/, { timeout: RESTART_TIMEOUT_MS });
    await expect.soft(settings.restartMessage).toBeHidden({ timeout: RESTART_TIMEOUT_MS });
  });

  await test.step('field outside "after restart": only "Saved", no button', async () => {
    await settings.saveChecksInterval();
    await expect.soft(settings.savedMessage).toHaveText('Saved', { timeout: SAVED_TIMEOUT_MS });
    await expect.soft(settings.restartButton).toBeHidden();
  });
});
});
