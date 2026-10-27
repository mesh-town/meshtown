import type { Page } from '@playwright/test';
import { test, expect } from '../fixtures/index';
import { contrastFailures, describeFailure, type Failure } from '../helpers/contrast';

const WIZARD = '.wizard-card';

// Height, corner radius and left margin of every button of a segment group on the current screen.
async function segmentBoxes(page: Page, seg: string): Promise<{ h: number; r: string; m: string }[]> {
  return page.locator(seg + ' .btn').evaluateAll((els) => els.map((e) => ({ h: e.getBoundingClientRect().height, r: getComputedStyle(e).borderTopLeftRadius, m: getComputedStyle(e).marginLeft })));
}

for (const theme of ['dark', 'light'] as const) {
  test(`81-contrast: ${theme}: setup screens, Networks and Home keep WCAG contrast (text 4.5, controls 3)`, async ({ app, page, targetKind }) => {
    test.skip(targetKind === 'node', 'the setup screens are shown the same way on a real node; the demo is enough');
    await page.emulateMedia({ colorScheme: theme });
    const wizard = app.wizard;
    const failures: Failure[] = [];
    const measure = async (root: string, where: string): Promise<void> => {
      await page.mouse.move(0, 0);
      failures.push(...(await contrastFailures(page, root, where, theme)));
    };
    await wizard.open();

    await test.step('scope screen: the progress line, the tiles and the skip link', async () => {
      await wizard.expectStep('scope');
      await measure(WIZARD, 'scope screen');
    });

    await test.step('system screen: the tiles (one pressed) and the Back and Next buttons', async () => {
      await wizard.tile('scope-system').click();
      await wizard.expectStep('os');
      await measure(WIZARD, 'system screen');
    });

    await test.step('browser screen: the tiles and the Back and Next buttons', async () => {
      await wizard.tile('os-mac').click();
      await wizard.expectStep('browser');
      await measure(WIZARD, 'browser screen');
    });

    await test.step('instruction: "More options" and the manual block expanded, a term hint open', async () => {
      await wizard.tile('browser-chrome').click();
      await wizard.expectStep('guide');
      await wizard.moreOptionsSummary.click();
      await app.proxyCard.manualToggle.click();
      await wizard.terms.first().focus();
      await expect(wizard.tooltip).toBeVisible();
      await measure(WIZARD, 'instruction');
    });

    await test.step('exit screen', async () => {
      await wizard.next();
      await wizard.expectStep('exit');
      await measure(WIZARD, 'exit screen');
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
  const wizard = app.wizard;
  await wizard.open();
  await wizard.tile('scope-system').click();
  await wizard.back();
  await wizard.expectStep('scope');
  const fill = (id: string): Promise<string> => page.locator(id).evaluate((el) => getComputedStyle(el).backgroundColor);
  expect(await fill('#scope-system')).not.toBe(await fill('#scope-browser'));
  await expect(wizard.tile('scope-system')).toHaveAttribute('aria-pressed', 'true');
});

test('83-contrast: scope, system and browser segments are separate buttons at least 36px tall', async ({ app, page, targetKind }) => {
  test.skip(targetKind === 'node', 'the demo is enough');
  const wizard = app.wizard;
  await wizard.open();
  const screens = [
    { seg: '#scope-seg', leave: 'scope-system' },
    { seg: '#os-seg', leave: 'os-mac' },
    { seg: '#browser-seg', leave: 'browser-chrome' },
  ] as const;
  for (const { seg, leave } of screens) {
    const boxes = await segmentBoxes(page, seg);
    expect(boxes.length, seg).toBeGreaterThan(1);
    for (const b of boxes) expect(b, seg).toMatchObject({ r: '7px', m: '0px' });
    for (const b of boxes) expect(b.h, seg).toBeGreaterThanOrEqual(36);
    await wizard.tile(leave).click();
  }
});
