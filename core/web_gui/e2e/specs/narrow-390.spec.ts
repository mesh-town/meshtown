import { test, expect } from '../fixtures/index';
import { DomainsSection } from '../pages/domains-section';

const WIDTH = 390;

test('95-narrow-tabs-bar: 390×844: the strip has no horizontal overflow and no "Mesh Town" logo, four tabs and "More ▾" fit the window, the menu opens the rare sections', async ({
  app,
}) => {
  await app.resize({ width: WIDTH, height: 844 });

  await test.step('#tabs: scrollWidth <= clientWidth', async () => {
    const { scrollWidth, clientWidth } = await app.tabs.scrollMetrics();
    expect.soft(scrollWidth, '#tabs has a horizontal overflow').toBeLessThanOrEqual(clientWidth);
  });

  await test.step('no "Mesh Town" text in #tabs', async () => {
    await expect.soft(app.tabs.root, 'the logo block is still in the strip').not.toContainText('Mesh Town');
  });

  await test.step('Home, Networks, Sites, Settings and "More ▾" are inside the window', async () => {
    await expect.soft(app.tabs.moreButton, 'the "More ▾" button').toHaveText('More ▾');
    for (const section of ['home', 'peers', 'domains', 'settings'] as const) {
      await expect.soft(app.tabs.tab(section), `tab ${section} is visible`).toBeVisible();
      expect.soft(await app.right(app.tabs.tab(section)), `tab ${section} goes beyond the window`).toBeLessThanOrEqual(WIDTH);
    }
    expect.soft(await app.right(app.tabs.moreButton), 'the "More ▾" button goes beyond the window').toBeLessThanOrEqual(WIDTH);
  });

  await test.step('the menu lists Diagnostics, Log and stays inside the window', async () => {
    await app.tabs.moreButton.click();
    await expect.soft(app.tabs.menuItems, 'menu items').toHaveText(['Diagnostics', 'Log']);
    expect.soft(await app.right(app.tabs.menu), 'the menu goes beyond the window').toBeLessThanOrEqual(WIDTH);
  });

  await test.step('choosing "Log" closes the menu, opens the section and renames the button', async () => {
    await app.tabs.menuItems.filter({ hasText: 'Log' }).click();
    await app.tabs.expectShown('log');
    await expect.soft(app.tabs.moreButton, 'the button is named after the section').toHaveText('Log ▾');
    await expect.soft(app.tabs.menu, 'the menu stays open after a choice').toBeHidden();
  });
});

test('96-narrow-sites-cards: 390×844: on "Sites" the page is not wider than the window and every "Remove" button is within it', async ({
  app,
  page,
  targetKind,
}) => {
  test.skip(targetKind === 'node', 'a real node has no domain fixtures');
  await app.resize({ width: WIDTH, height: 844 });
  await app.tabs.open('domains');
  const domains = new DomainsSection(page);
  await expect(domains.cardRecheckButton, 'no site card: the section is not mounted').toBeVisible({ timeout: 10000 });

  const scroll = await page.evaluate(() => document.scrollingElement?.scrollWidth ?? 0);
  expect.soft(scroll, 'the page is wider than the window').toBeLessThanOrEqual(WIDTH);
  const removes = await domains.cardRemoveButtons.all();
  expect.soft(removes.length, 'cards with a "Remove" button').toBeGreaterThan(0);
  for (const remove of removes) {
    const right = await domains.rightEdge(remove);
    expect.soft(right, `a "Remove" button goes beyond the window: right edge ${right}`).toBeLessThanOrEqual(WIDTH);
  }
});
