import type { Locator } from '@playwright/test';
import { test, expect } from '../fixtures/index';

const MIN_GAP = 8;

// The vertical space between the paragraph of the card and the button or link below it.
async function gapBelowText(card: Locator): Promise<number> {
  const text = await card.locator('p').boundingBox();
  const action = await card.locator('.btn').boundingBox();
  if (!text || !action) throw new Error('the card has no paragraph or no action');
  return action.y - (text.y + text.height);
}

test.describe('excluded card spacing', () => {
  test.skip(({ targetKind }) => targetKind === 'node', 'starting banned is a flag of the demo node; skipped before the target starts');
  test.use({ targetFlags: { extraArgs: ['--me-excluded'] } });

  test('112-excluded-card-gap: the "Join with another invite" button keeps a gap from the text above it', async ({ app, page }) => {
    const card = page.locator('#members-removed-card');
    await app.tabs.open('peers');
    await expect(card).toBeVisible();
    expect(await gapBelowText(card)).toBeGreaterThanOrEqual(MIN_GAP);
  });

  test.describe('in the app', () => {
    test.use({ userAgent: 'Mozilla/5.0 (Linux; Android 14) MeshTownApp/Android' });

    test('113-excluded-card-app: the reset link keeps a gap and the card speaks of the phone', async ({ app, page }) => {
        const card = page.locator('#members-removed-card');
      await app.tabs.open('peers');
      await expect(card.getByRole('button', { name: /Leave network and reset/ })).toBeVisible();
      await expect(card.getByRole('heading')).toContainText('This phone');
      expect(await gapBelowText(card)).toBeGreaterThanOrEqual(MIN_GAP);
    });
  });
});
