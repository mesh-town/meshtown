import { test, expect } from '../fixtures/index';
import { NetSubTabs } from '../pages/net-subtabs';
import { PortsSection } from '../pages/ports-section';

const LONG_ERROR = 'x'.repeat(40) + ' ' + 'y'.repeat(60) + ' ' + 'z'.repeat(56);

test('ports-long-error: at 1200 a 160-character error wraps inside the cell and the table does not scroll sideways (demo)', async ({
  app,
  page,
  targetKind,
}) => {
  test.skip(targetKind === 'node', 'the shared port row is a web_demo stub fixture; a real node has none');
  const ports = new PortsSection(page);
  await app.resize({ width: 1200, height: 800 });
  await app.tabs.open('peers');
  await new NetSubTabs(page).open('services');
  const row = ports.netRows.first();
  const table = page.getByRole('table', { name: 'Ports shared with you' });
  await page.route('**/api/config', async (route) => {
    if (route.request().method() !== 'PUT') return route.fallback();
    await route.fulfill({ status: 500, json: { error: 'internal', message: LONG_ERROR } });
  });

  await test.step('"Open" fails with a long error', async () => {
    await ports.openButton(row).click();
    await expect.soft(row.locator('.field-error-text')).toContainText('z'.repeat(56));
  });

  await test.step('.table-scroll has no horizontal overflow', async () => {
    const scroll = page.locator('.table-scroll').filter({ has: table });
    const m = await scroll.evaluate((el) => ({ scrollWidth: el.scrollWidth, clientWidth: el.clientWidth }));
    expect.soft(m.scrollWidth, 'sideways scroll in the table').toBeLessThanOrEqual(m.clientWidth);
  });

  await test.step('the "Open on this computer" button stays in the visible part of the table', async () => {
    const right = await app.right(ports.openButton(row));
    const scrollRight = await page.locator('.table-scroll').filter({ has: table }).evaluate((el) => el.getBoundingClientRect().right);
    expect.soft(right, 'the button goes beyond the table').toBeLessThanOrEqual(scrollRight);
  });
});
