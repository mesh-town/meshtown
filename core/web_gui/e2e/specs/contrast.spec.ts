import type { Page } from '@playwright/test';
import { test, expect } from '../fixtures/index';
import { NetsBlock } from '../pages/home/nets-block';
import { contrastFailures, describeFailure, type Failure } from '../helpers/contrast';

const WIZARD = '.wizard-card';

// The current step title is a disabled button; a click is only needed to move to another step.
async function openStep(page: Page, n: 2 | 3): Promise<void> {
  const title = page.locator('#wizard-step-' + n);
  if (await title.isEnabled()) await title.click();
}

for (const theme of ['dark', 'light'] as const) {
  test(`81-contrast: ${theme}: setup steps, Networks and Home keep WCAG contrast (text 4.5, controls 3)`, async ({ app, page, targetKind }) => {
    test.skip(targetKind === 'node', 'the setup steps are shown the same way on a real node; the demo is enough');
    await page.emulateMedia({ colorScheme: theme });
    const failures: Failure[] = [];
    const measure = async (root: string, where: string): Promise<void> => {
      await page.mouse.move(0, 0);
      failures.push(...(await contrastFailures(page, root, where, theme)));
    };
    await new NetsBlock(page).openWizard();

    await test.step('browser: system scope, manual block and "More options" expanded', async () => {
      await openStep(page, 2);
      await app.proxyCard.pickScope('system');
      await app.proxyCard.manualToggle.click();
      await page.locator('#more-options > summary').click();
      await measure(WIZARD, 'browser step');
    });

    await test.step('exit', async () => {
      await openStep(page, 3);
      await measure(WIZARD, 'exit step');
    });

    await test.step('Networks tab and Home', async () => {
      await app.tabs.open('peers');
      await measure('#sec-peers', 'Networks tab');
      await app.tabs.open('home');
      await measure('#sec-home', 'Home');
    });

    expect(failures.map(describeFailure), `contrast failures in the ${theme} theme`).toEqual([]);
  });
}

test('82-contrast: scope buttons differ by fill', async ({ app, page, targetKind }) => {
  test.skip(targetKind === 'node', 'the demo is enough');
  await new NetsBlock(page).openWizard();
  await openStep(page, 2);
  await app.proxyCard.pickScope('system');
  const fill = (id: string): Promise<string> => page.locator(id).evaluate((el) => getComputedStyle(el).backgroundColor);
  expect(await fill('#scope-system')).not.toBe(await fill('#scope-browser'));
  await expect(page.locator('#scope-system')).toHaveAttribute('aria-pressed', 'true');
});

test('83-contrast: scope, system and browser segments are separate buttons at least 36px tall', async ({ app, page, targetKind }) => {
  test.skip(targetKind === 'node', 'the demo is enough');
  await new NetsBlock(page).openWizard();
  await app.proxyCard.pickScope('system');
  await app.proxyCard.manualToggle.click();
  for (const seg of ['#scope-seg', '#os-seg', '#browser-seg']) {
    const boxes = await page.locator(seg + ' .btn').evaluateAll((els) => els.map((e) => ({ h: e.getBoundingClientRect().height, r: getComputedStyle(e).borderTopLeftRadius, m: getComputedStyle(e).marginLeft })));
    expect(boxes.length, seg).toBeGreaterThan(1);
    for (const b of boxes) expect(b, seg).toMatchObject({ r: '7px', m: '0px' });
    for (const b of boxes) expect(b.h, seg).toBeGreaterThanOrEqual(36);
  }
});
