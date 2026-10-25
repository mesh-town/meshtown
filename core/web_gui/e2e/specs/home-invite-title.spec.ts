import { test, expect } from '../fixtures/index';

const SIZES = [
  { width: 1100, height: 800 },
  { width: 1280, height: 800 },
  { width: 1600, height: 900 },
];
const COUNT_TEXT = 'You and 2 others, both online';

test.describe('home-invite-title', () => {
  test.skip(({ targetKind }) => targetKind === 'demo', 'the demo node has no "Network invite" card');

  for (const size of SIZES) {
    test(`77-invite-title-${size.width}: the member counter does not cover the "Network invite" title`, async ({ app, page }) => {
      await app.resize(size);
      const card = page.locator('#home-invite');
      await expect(card).toBeVisible();
      await page.locator('#home-members-count').evaluate((el, text) => {
        el.textContent = text;
        (el as HTMLElement).hidden = false;
      }, COUNT_TEXT);
      const apart = await page.evaluate(() => {
        const title = document.querySelector('#home-invite-title')!.getBoundingClientRect();
        const count = document.querySelector('#home-members-count')!.getBoundingClientRect();
        return count.left >= title.right || count.top >= title.bottom;
      });
      expect(apart, 'the counter must not overlap the title').toBe(true);
    });
  }
});
