import { test, expect } from '../fixtures/index';

interface ConfigReply {
  rev?: number;
  config?: { socks?: { port?: number } };
  file?: { message?: string };
}
interface StateReply {
  socks?: { listen?: string; strict?: boolean };
  node?: { identity?: string; identity_reason?: string };
}

test.describe('9-cli-override-value: --cli-override socks.port=2000: the file and the effective value differ, saving changes only the file (WEB-API-config.md 1.6-1.7)', () => {
  test.use({ targetFlags: { socksOverride: true } });

  test('9-cli-override-value: the file and the effective value', async ({ api, app, target }) => {
    const socksPort = String(target.socksPort);
    let rev: number | undefined;

    await test.step('GET /api/config: the file socks.port is untouched by the flag (1080)', async () => {
      const cfg = await api.getJson<ConfigReply>('/api/config');
      rev = cfg.body?.rev;
      expect.soft(cfg.status, 'status').toBe(200);
      expect.soft(cfg.body?.config?.socks?.port).toBe(1080);
    });

    const listenOf = async (): Promise<[number, string | undefined]> => {
      const state = await api.getJson<StateReply>('/api/state');
      return [state.status, state.body?.socks?.listen];
    };

    await test.step('GET /api/state: the effective socks.listen = the flag value (the target free port)', async () => {
      // socks.listen does not appear right after the target starts: wait for the value instead of reading once
      await expect.poll(listenOf, { timeout: 10000, intervals: [50, 100, 200] }).toEqual([200, `127.0.0.1:${socksPort}`]);
    });

    await test.step('PUT /api/config 1090: the file accepted the new value', async () => {
      expect(rev, 'rev from GET /api/config').toBeDefined();
      const put = await api.putJson<ConfigReply>('/api/config', { rev, config: { socks: { port: 1090 } } });
      expect.soft(put.status, 'status').toBe(200);
      expect.soft(put.body?.config?.socks?.port).toBe(1090);
    });

    await test.step('GET /api/state after saving: the effective value is still the flag value', async () => {
      // socks.listen does not appear right after the target starts: wait for the value instead of reading once
      await expect.poll(listenOf, { timeout: 10000, intervals: [50, 100, 200] }).toEqual([200, `127.0.0.1:${socksPort}`]);
    });

    await test.step('the page shows the effective flag value, different from the file value', async () => {
      // the page is already opened by the fixture: the note is re-read from the snapshot without rebuilding the form
      await app.tabs.open('settings');
      await expect.soft(app.settings.root).toContainText(socksPort);
    });

    await test.step('#field-web-enabled: a locked toggle instead of the text "on"', async () => {
      await app.settings.openExpert();
      await expect.soft(app.settings.webEnabled).toBeDisabled();
      await expect.soft(app.settings.staleEnabledText).toHaveCount(0);
    });
  });
});

test.describe('10-identity: --identity persistent/declined/file_problem: node.identity+identity_reason (WEB-API.md 1.3), the reason is taken from file.message', () => {
  // declined/file_preserved are unreachable with this demo node; persistent
  // is checked by a separate test, not by this demo node emulation.
  // The real node does not reproduce it.
  test.skip(
    ({ targetKind }) => targetKind === 'node',
    'not reproducible by a real node: meshtown run has no such flag',
  );

  test.describe('api persistent', () => {
    test.use({ targetFlags: { extraArgs: ['--identity', 'persistent'] } });

    test('10-identity: API --identity persistent', async ({ api }) => {
      await test.step("--identity persistent: node.identity='persistent', identity_reason=None", async () => {
        const state = await api.getJson<StateReply>('/api/state');
        expect.soft(state.status, 'status').toBe(200);
        expect.soft(state.body?.node?.identity).toBe('persistent');
        expect.soft(state.body?.node?.identity_reason ?? undefined).toBeUndefined();
      });
    });
  });

  for (const reason of ['declined', 'file_problem', 'file_preserved']) {
    test.describe(`api ${reason}`, () => {
      test.use({ targetFlags: { extraArgs: ['--identity', reason] } });

      test(`10-identity: API --identity ${reason}`, async ({ api }) => {
        await test.step(`--identity ${reason}: node.identity='temporary', identity_reason='${reason}'`, async () => {
          const state = await api.getJson<StateReply>('/api/state');
          expect.soft(state.status, 'status').toBe(200);
          expect.soft(state.body?.node?.identity).toBe('temporary');
          expect.soft(state.body?.node?.identity_reason).toBe(reason);
        });

        await test.step(`--identity ${reason}: the reason for the human is read from file.message`, async () => {
          const cfg = await api.getJson<ConfigReply>('/api/config');
          expect.soft(cfg.status, 'status').toBe(200);
          expect.soft(cfg.body?.file?.message ?? '', 'file.message').not.toBe('');
        });
      });
    });
  }
});

test('socks-strict: the strict-mode switch in Settings is saved and the node state reports it (WEB-API-config.md, socks.strict)', async ({
  app,
  api,
}) => {
  const strictOf = async (): Promise<boolean | undefined> => {
    const state = await api.getJson<StateReply>('/api/state');
    return state.body?.socks?.strict;
  };

  await test.step('GET /api/state: socks.strict is not on by default', async () => {
    expect.soft((await strictOf()) ?? false).toBe(false);
  });

  await test.step('Settings: the switch is turned on and saved', async () => {
    await app.tabs.open('settings');
    await app.settings.socksStrict.check();
    await app.settings.saveButton.click();
    await expect.soft(app.settings.savedMessage).toBeVisible();
  });

  await test.step('GET /api/state: socks.strict is true', async () => {
    await expect.poll(strictOf, { timeout: 10000, intervals: [50, 100, 200] }).toBe(true);
  });
});

test('43-settings-dirty-marker-and-intro: the unsaved-settings marker is readable without hovering — text, not only color; one shared restart hint instead of a repeat on every field', async ({
  app,
  page,
}) => {
  await app.tabs.open('settings');

  await test.step('#sec-settings: the intro text explains "after restart" and the "Restart Mesh Town" button', async () => {
    await expect.soft(app.settings.intro).toBeVisible();
  });

  await test.step('#tab-settings.has-dirty::after: the text is readable without hovering ("not saved")', async () => {
    await app.settings.typeSocksPort('1090');
    await expect.soft(app.settings.tab).toHaveClass(/has-dirty/);
    expect.soft(await app.settings.tabAfterContent()).toContain('not saved');
  });

  await test.step('"after restart" badge: grey tokens, not orange', async () => {
    const grey = await app.settings.greyTokens();
    await expect.soft(app.settings.restartBadge).toHaveCSS('background-color', grey.bg);
    await expect.soft(app.settings.restartBadge).toHaveCSS('color', grey.fg);
  });

  await test.step('editing a General field while the Advanced settings are collapsed — the sticky save bar in a 1280×720 window', async () => {
    await page.setViewportSize({ width: 1280, height: 720 });
    await app.settings.checksEnabled.click();
    await expect.soft(app.settings.saveBar).toBeInViewport();
    await app.settings.resetButton.click();
  });
});
