import { test, expect } from '../fixtures/index';

// Expected texts (the node target takes the same ones as the demo).
const EXPECTED_4A = "Couldn't parse JSON";
const EXPECTED_4B = 'not readable';
const FIELD_PATH = 'socks.port';
const EXPECTED_5 = `Proxy port (SOCKS5 and HTTP) (${FIELD_PATH}): file has 80`;

test.describe('--file-state invalid', () => {
  test.use({ targetFlags: { extraArgs: ['--file-state', 'invalid'] } });

  test('4-file-invalid: --file-state invalid: the banner is there, the page is alive', async ({ app }) => {
    await app.tabs.open('settings');

    await test.step('the file error banner is there', async () => {
      await expect.soft(app.fileBanner.brokenTitle, 'the file error banner is there').toBeVisible();
      await expect.soft(app.fileBanner.brokenMessage(EXPECTED_4A)).toBeVisible();
    });

    await test.step('the page is alive (the "Settings" tab is visible)', async () => {
      await expect.soft(app.tabs.panel('settings')).toBeVisible();
    });
  });
});

test.describe('--file-state unreadable', () => {
  test.use({ targetFlags: { extraArgs: ['--file-state', 'unreadable'] } });

  test('4-file-unreadable: --file-state unreadable: the banner is there, the page is alive', async ({ app }) => {
    await app.tabs.open('settings');

    await test.step('the file error banner is there', async () => {
      await expect.soft(app.fileBanner.brokenTitle, 'the file error banner is there').toBeVisible();
      await expect.soft(app.fileBanner.brokenMessage(EXPECTED_4B)).toBeVisible();
    });

    await test.step('the page is alive (the "Settings" tab is visible)', async () => {
      await expect.soft(app.tabs.panel('settings')).toBeVisible();
    });
  });
});

test.describe('--field-error socks.port', () => {
  test.use({ targetFlags: { extraArgs: ['--field-error', FIELD_PATH] } });

  test('5-field-error: --field-error socks.port: the error is visible next to the field', async ({ app }) => {
    await app.tabs.open('settings');

    await test.step(`the error next to field ${FIELD_PATH} is visible`, async () => {
      await expect.soft(app.fileBanner.fieldErrorsHint).toBeVisible();
      await expect.soft(app.fileBanner.fieldError(EXPECTED_5)).toBeVisible();
    });
  });
});
