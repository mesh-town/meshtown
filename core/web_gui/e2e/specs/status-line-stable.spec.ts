import { test, expect } from '../fixtures/index';
import { putRouteExits, type Peer } from '../helpers/route-exits-api';

const SETTLE_MS = 10000;

// The header status line is one line and does not change the height of the bar when an exit is chosen
// (the route phrase changes, the line stays short and the vote badge stays on the same row).
for (const viewport of [{ width: 1280, height: 800 }, { width: 390, height: 844 }]) {
  test.describe(`status-line-stable ${viewport.width}`, () => {
    test.skip(({ targetKind }) => targetKind !== 'demo', 'members with an exit are needed: only the demo provides them');
    test.use({ viewport });

    test(`status-line-stable: at ${viewport.width}px the status line is one line and the bar keeps its height after choosing an exit`, async ({ app, api, page }) => {
      const { body } = await api.getJson<{ peers?: Peer[] }>('/api/state');
      const exit = (body?.peers ?? []).find((p) => !p.self && p.exit && p.online);
      test.skip(!exit, 'no member with an exit in the demo snapshot');
      const line = app.topPanel.statusLine;
      const measure = () =>
        page.evaluate(() => {
          const status = document.querySelector('#status-line') as HTMLElement;
          const bar = document.querySelector('.topbar') as HTMLElement;
          const style = getComputedStyle(status);
          const lineHeight = parseFloat(style.lineHeight) || parseFloat(style.fontSize) * 1.5;
          return {
            statusHeight: status.getBoundingClientRect().height,
            statusRight: status.getBoundingClientRect().right,
            barHeight: bar.getBoundingClientRect().height,
            lineHeight,
            window: window.innerWidth,
          };
        });

      try {
        await expect(line).toBeVisible({ timeout: SETTLE_MS });
        const before = await measure();
        expect(before.statusHeight, 'the status line is one line before the click').toBeLessThan(before.lineHeight * 2);

        await app.exitCard.choiceById(exit!.id_full).click();
        await expect(line).toContainText('Sites open via', { timeout: SETTLE_MS });
        const after = await measure();

        expect(after.statusHeight, 'the status line is one line after the click').toBeLessThan(after.lineHeight * 2);
        expect(after.barHeight, 'the height of the top bar does not change').toBe(before.barHeight);
        expect(after.statusRight, 'the line is not wider than the window').toBeLessThanOrEqual(after.window);
      } finally {
        await putRouteExits(api, []);
      }
    });
  });
}
