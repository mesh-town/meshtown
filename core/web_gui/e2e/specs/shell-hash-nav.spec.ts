import { test, expect } from '../fixtures/index';

test('68-shell-hash-nav: address #log selects "Log", "Back" returns to "Home"; #overview and a junk hash give "Home"; after a change the page is scrolled to the top', async ({
  app,
  page,
  target,
}) => {
  const at = (hash: string) => page.goto(`${target.origin}/${hash}`);
  const scrollY = () => page.evaluate(() => window.scrollY);

  await test.step('go to #log — the "Log" tab is selected', async () => {
    await at('#log');
    await app.tabs.expectShown('log');
  });

  await test.step('"Back" — "Home"', async () => {
    await page.goBack();
    await app.tabs.expectShown('home');
  });

  await test.step('#overview — "Home" (the old link)', async () => {
    await at('#log');
    await app.tabs.expectShown('log');
    await at('#overview');
    await app.tabs.expectShown('home');
  });

  await test.step('#nope — "Home"', async () => {
    await at('#log');
    await app.tabs.expectShown('log');
    await at('#nope');
    await app.tabs.expectShown('home');
  });

  await test.step('after switching section the scroll returned to the top', async () => {
    // a short window: otherwise the whole page fits and there is nothing to scroll — the step would check nothing
    await page.setViewportSize({ width: 1280, height: 300 });
    await page.evaluate(() => window.scrollTo(0, 500));
    await expect.poll(scrollY).toBeGreaterThan(0);
    await at('#peers');
    await app.tabs.expectShown('peers');
    await expect.poll(scrollY).toBe(0);
  });
});
