import { test, expect } from '../fixtures/index';

const SETTLE_MS = 25000;
const TITLE = /This computer can't connect to ".+"/;

test.describe('admission-refused', () => {
  test.skip(({ targetKind }) => targetKind === 'node', 'a refused admission is a flag of the demo node; skipped before the target starts');

  test.describe('without the flag', () => {
    test('admission-refused-off: no card and no red "Sites won\'t open" in the header', async ({ app, page }) => {
      await expect(app.topPanel.statusLine).toBeVisible();
      await expect(page.locator('#admission-refused')).toHaveCount(0);
      await expect(app.topPanel.statusLine).not.toContainText("Sites won't open");
    });
  });

  test.describe('with --admission-refused', () => {
    test.use({ targetFlags: { extraArgs: ['--admission-refused'] } });

    test('admission-refused-on: the card and the red header appear after a delay, not at once', async ({ app, page }) => {
      // The card is in the document once: on the shown tab.
      const card = page.locator('#admission-refused');
      await expect(card).toHaveCount(0);
      await expect(app.topPanel.statusLine).not.toContainText("Sites won't open");

      await expect(card).toBeVisible({ timeout: SETTLE_MS });
      await expect(card.getByRole('heading')).toHaveText(TITLE);
      await expect(app.topPanel.statusLine).toContainText("Sites won't open");
      await expect(app.topPanel.statusLine.locator('.dot')).toHaveClass(/dot-red/);
    });
  });
});
