import type { Page } from '@playwright/test';
import { test, expect } from '../fixtures/index';

// Two networks in one node: the demo is started with `--nets 2`. The real node project has one network, so it is skipped here.

const WIDTHS = [
  { name: 'narrow 390', width: 390, height: 844 },
  { name: 'wide 1280', width: 1280, height: 800 },
] as const;

// The slot ids come from the page: the second network tab is the last slot tab before "+".
const slotTabs = (page: Page) => page.locator('#peers-net-tabs a.ntab[data-slot]');
const secondSlot = async (page: Page) => (await slotTabs(page).nth(1).getAttribute('data-slot'))!;
const tabs = (page: Page) => page.locator('#peers-net-tabs');
const tab = (page: Page, slot: string) => tabs(page).locator(`[data-slot="${slot}"]`);
const addTab = (page: Page) => tabs(page).locator('a.ntab.add');
const hash = (page: Page) => page.evaluate(() => location.hash);

test.describe('two networks (demo --nets 2)', () => {
  test.skip(({ targetKind }) => targetKind === 'node', 'a real node has one network');
  test.use({ targetFlags: { extraArgs: ['--nets', '2'] } });

  for (const vp of WIDTHS) {
    test.describe(vp.name, () => {
      test.use({ viewport: { width: vp.width, height: vp.height } });

      test(`multi-net-members-row-${vp.width}: Members shows the tab row and "+"; the second tab sets #peers/<slot>, "Back" returns to the first`, async ({ page, app, target }) => {
        await test.step('given: Members is open', async () => {
          await app.open(target.origin, target.token);
          await page.goto(`${target.origin}/#peers`);
          await app.tabs.expectShown('peers');
        });

        await test.step('the row has two network tabs and the "+" tab', async () => {
          await expect(tabs(page)).toBeVisible();
          await expect(tabs(page).getByRole('link')).toHaveCount(3);
          await expect(addTab(page)).toBeVisible();
          await expect(slotTabs(page)).toHaveCount(2);
          await expect(slotTabs(page).nth(1)).toBeVisible();
        });

        const firstSlot = (await slotTabs(page).first().getAttribute('data-slot'))!;
        const SECOND = await secondSlot(page);

        await test.step('clicking the second tab: the hash is #peers/<slot> and the tab is selected', async () => {
          await tab(page, SECOND).click();
          await expect.poll(() => hash(page)).toBe(`#peers/${SECOND}`);
          await expect(tab(page, SECOND)).toHaveAttribute('aria-current', 'page');
        });

        await test.step('"Back" selects the first network again', async () => {
          await page.goBack();
          await app.tabs.expectShown('peers');
          await expect(tab(page, firstSlot)).toHaveAttribute('aria-current', 'page');
          await expect(tab(page, SECOND)).not.toHaveAttribute('aria-current', 'page');
        });
      });

      test(`multi-net-add-tab-${vp.width}: "+" opens the "Add another network" form, "Cancel" returns to a network tab`, async ({ page, app, target }) => {
        await app.open(target.origin, target.token);
        await page.goto(`${target.origin}/#peers`);
        await app.tabs.expectShown('peers');

        await test.step('"+" opens the form', async () => {
          await addTab(page).click();
          await expect.poll(() => hash(page)).toBe('#peers/add');
          await expect(page.getByRole('heading', { name: 'Add another network' })).toBeVisible();
          await expect(page.locator('#add-net-input:visible')).toBeVisible();
        });

        await test.step('"Cancel" closes the form and leaves the "+" tab', async () => {
          await page.locator('#add-net-cancel:visible').click();
          await expect(page.locator('#add-net-card:visible')).toBeHidden();
          await expect.poll(() => hash(page)).toMatch(/^#peers(\/|$)/);
          await expect.poll(() => hash(page)).not.toBe('#peers/add');
          await expect(addTab(page)).not.toHaveAttribute('aria-current', 'page');
        });
      });

      test(`multi-net-home-settings-${vp.width}: Home lists the networks and "Open Members" leads to the tab; Settings has "Add another network" and no join card`, async ({ page, app, target }) => {
        await app.open(target.origin, target.token);

        await test.step('Home shows a row per network; "Open Members" goes to that network tab', async () => {
          await app.tabs.expectShown('home');
          const rows = page.locator('#home-nets .net-row');
          await expect(rows).toHaveCount(2);
          const SECOND = (await rows.nth(1).getAttribute('data-slot'))!;
          await rows.nth(1).locator('a.net-row-open').click();
          await app.tabs.expectShown('peers');
          await expect.poll(() => hash(page)).toBe(`#peers/${SECOND}`);
          await expect(tab(page, SECOND)).toHaveAttribute('aria-current', 'page');
        });

        await test.step('Settings: the add-network card is there, the join card is not', async () => {
          await page.goto(`${target.origin}/#settings`);
          await app.tabs.expectShown('settings');
          await expect(page.locator('#add-net-card:visible')).toBeVisible();
          await expect(page.getByRole('heading', { name: 'Add another network' })).toBeVisible();
          await expect(page.locator('#join-card')).toHaveCount(0);
        });
      });
    });
  }
});

test('multi-net-single-network: with one network Members has no tab row', async ({ page, app, target }) => {
  await app.open(target.origin, target.token);
  await page.goto(`${target.origin}/#peers`);
  await app.tabs.expectShown('peers');
  await expect(page.locator('#peers-list')).toBeVisible();
  await expect(tabs(page)).toHaveCount(0);
});
