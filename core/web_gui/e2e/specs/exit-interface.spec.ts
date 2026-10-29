import { test, expect } from '../fixtures/index';
import { NetSubTabs } from '../pages/net-subtabs';

// The field "Send exit traffic through" in Device settings: a choice is saved to the config and survives a reload;
// a bad value of `exit.interface` in the settings file is reported next to the other file errors.

const SELECT = '#you-exit-iface';
const STATUS = '#you-exit-iface-status';
const SAVED = 'Saved';
// The first item is "Any network"; the names after it are the networks of the machine under test (real names on a node).
const FIRST_NETWORK_INDEX = 1;
const FIELD_PATH = 'exit.interface';

test('exit-interface-save: the first network after "Any" is picked, saved, and still chosen after a reload', async ({ app, page }) => {
  const subs = new NetSubTabs(page);
  const select = page.locator(SELECT);
  await app.tabs.open('peers');
  await subs.open('device');
  await expect(select).toBeVisible();

  const values = await select.locator('option').evaluateAll((items) => items.map((item) => (item as HTMLOptionElement).value));
  expect(values[0], 'the first item is "Any network"').toBe('');
  expect(values.length, `the list has a network after "Any": ${JSON.stringify(values)}`).toBeGreaterThan(FIRST_NETWORK_INDEX);
  const picked = values[FIRST_NETWORK_INDEX];

  await test.step('when a network is picked, then the page says it is saved', async () => {
    await select.selectOption(picked);
    await expect.soft(page.locator(STATUS)).toHaveText(SAVED);
    await expect.soft(select).toHaveValue(picked);
  });

  await test.step('when the page is reloaded, then the same network is still chosen', async () => {
    await page.reload();
    await app.tabs.open('peers');
    await subs.open('device');
    await expect.soft(page.locator(SELECT)).toHaveValue(picked);
  });
});

test.describe(`--field-error ${FIELD_PATH}`, () => {
  // The node target only knows a fixed set of launch flags; the bad value of this field comes from the demo.
  test.skip(({ targetKind }) => targetKind === 'node', 'the bad-value table of --field-error is a web_demo feature');
  test.use({ targetFlags: { extraArgs: ['--field-error', FIELD_PATH] } });

  test('exit-interface-field-error: --field-error exit.interface: the error is visible next to the other file errors', async ({ app }) => {
    await app.tabs.open('settings');

    await test.step(`the error for ${FIELD_PATH} is listed in the banner`, async () => {
      await expect.soft(app.fileBanner.fieldErrorsHint).toBeVisible();
      await expect.soft(app.fileBanner.fieldError(`${FIELD_PATH}: file has`)).toBeVisible();
    });
  });
});
