import type { Page } from '@playwright/test';
import { test, expect } from '../fixtures/index';
import { NetSubTabs } from '../pages/net-subtabs';
import { PortsSection } from '../pages/ports-section';

// Two networks in one node: the demo is started with `--nets 2`. The real node project has one network, so it is skipped here.

const slotTabs = (page: Page) => page.locator('#peers-net-tabs a.ntab[data-slot]');
const slotOf = async (page: Page, index: number) => (await slotTabs(page).nth(index).getAttribute('data-slot'))!;
const hash = (page: Page) => page.evaluate(() => location.hash);

test.describe('Networks tab (demo --nets 2)', () => {
  test.skip(({ targetKind }) => targetKind === 'node', 'a real node has one network');
  test.use({ targetFlags: { extraArgs: ['--nets', '2'] } });

  test.beforeEach(async ({ app, page, target }) => {
    await app.open(target.origin, target.token);
    await page.goto(`${target.origin}/#peers`);
    await app.tabs.expectShown('peers');
  });

  test('networks-tab-three-sections: a network tab has Members, Shared services (ports) and Device settings; each opens its own address', async ({ page }) => {
    const subs = new NetSubTabs(page);
    const first = await slotOf(page, 0);

    await test.step('three sub-tabs, Members selected', async () => {
      await expect(subs.root).toBeVisible();
      await expect(subs.root.getByRole('tab')).toHaveText(['Members', 'Shared services (ports)', 'Device settings']);
      await expect(subs.tab('members')).toHaveAttribute('aria-selected', 'true');
      await expect(page.locator('#peers-list')).toBeVisible();
    });

    await test.step('Shared services: the hash gets the tail and the ports are shown', async () => {
      await subs.open('services');
      await expect.poll(() => hash(page)).toBe(`#peers/${first}/services`);
      await expect(page.getByRole('table', { name: 'My published ports' })).toBeVisible();
      await expect(page.locator('#peers-list')).toHaveCount(0);
    });

    await test.step('Device settings: the hash gets the tail', async () => {
      await subs.open('device');
      await expect.poll(() => hash(page)).toBe(`#peers/${first}/device`);
      await expect(page.locator('#peers-list')).toHaveCount(0);
    });

    await test.step('Members: no tail', async () => {
      await subs.open('members');
      await expect.poll(() => hash(page)).toBe(`#peers/${first}`);
    });
  });

  test('networks-tab-keeps-services: switching network keeps Shared services', async ({ page }) => {
    const subs = new NetSubTabs(page);
    const second = await slotOf(page, 1);

    await subs.open('services');
    await page.locator(`#peers-net-tabs [data-slot="${second}"]`).click();
    await expect.poll(() => hash(page)).toBe(`#peers/${second}/services`);
    await expect(subs.tab('services')).toHaveAttribute('aria-selected', 'true');
    await expect(page.getByRole('table', { name: 'My published ports' })).toBeVisible();
  });

  test('networks-tab-publish-in-b: "Publish port" in the second network lists the port there and not in the first', async ({ page }) => {
    const subs = new NetSubTabs(page);
    const ports = new PortsSection(page);
    const first = await slotOf(page, 0);
    const second = await slotOf(page, 1);
    const open = async (slot: string) => {
      await page.goto(`${page.url().split('#')[0]}#peers/${slot}/services`);
      await expect(subs.tab('services')).toHaveAttribute('aria-selected', 'true');
    };

    await open(second);
    await ports.publish('8099', 'only-in-b');
    await expect(ports.ownBody).toContainText('8099', { timeout: 6000 });

    await open(first);
    await expect(ports.ownBody).not.toContainText('8099');

    await open(second);
    await expect(ports.ownBody).toContainText('8099');
    await ports.ownBody.getByRole('button', { name: 'Unpublish' }).click();
    await expect(ports.ownBody).not.toContainText('8099', { timeout: 6000 });
  });

  test('networks-tab-old-ports-hash: #ports opens Shared services', async ({ page, app, target }) => {
    const subs = new NetSubTabs(page);
    await page.goto(`${target.origin}/#ports`);
    await app.tabs.expectShown('peers');
    await expect(subs.tab('services')).toHaveAttribute('aria-selected', 'true');
    await expect(page.getByRole('table', { name: 'My published ports' })).toBeVisible();
  });

  test.describe('narrow 390', () => {
    test.use({ viewport: { width: 390, height: 844 } });

    test('networks-tab-subtabs-fit-390: subtabs fit at 390', async ({ page, app }) => {
      const subs = new NetSubTabs(page);
      await expect(subs.root).toBeVisible();
      for (const sub of ['members', 'services', 'device'] as const) {
        await expect(subs.tab(sub), `sub-tab ${sub}`).toBeVisible();
        expect.soft(await app.right(subs.tab(sub)), `sub-tab ${sub} goes beyond the window`).toBeLessThanOrEqual(390);
      }
      expect(await app.scrollWidth(), 'the page is wider than the window').toBeLessThanOrEqual(390);
    });
  });
});
