import { test, expect } from '../fixtures/index';
import { WIZARD_TEXT } from '../pages/home/wizard';

// The step-by-step setup of a node that is already in a network: the questions on one page, then the exit screen.
// The proxy is on in the demo snapshot; on a real node it depends on startup flags.

test.describe('the setup wizard of a node in a network', () => {
  test.skip(({ targetKind }) => targetKind === 'node', 'the proxy is on in the demo node snapshot; on a real node it depends on startup flags');

  test('125-wizard-steps: all apps → instruction opens under the answers → Next → exit → Finish: the progress counts 2, 4, 5 of 5, the instruction shows the address, Finish opens Overview and keeps the done mark', async ({ app, page }) => {
    const wizard = app.wizard;

    await test.step('given: a node in a network, the Setup tab is open — only the scope question, step 2 of 5', async () => {
      await wizard.open();
      await wizard.expectCursor('scope');
      await expect.soft(wizard.rows).toHaveCount(1);
      await expect.soft(wizard.rowHeading('scope')).toHaveText(WIZARD_TEXT.scopeQuestion);
      await expect.soft(wizard.progress).toHaveText('Step 2 of 5');
    });

    await test.step('when "All apps" is chosen, then the system and instruction rows open under it, step 4 of 5', async () => {
      await wizard.tile('scope-system').click();
      await wizard.expectCursor('guide');
      await expect.soft(wizard.rows).toHaveCount(3);
      await expect.soft(wizard.rowHeading('os')).toHaveText(WIZARD_TEXT.osQuestion);
      await expect.soft(wizard.rows.filter({ has: page.locator('#browser-seg') })).toHaveCount(0);
      await expect.soft(wizard.progress).toHaveText('Step 4 of 5');
      await expect.soft(wizard.nextButton).toBeVisible();
    });

    await test.step('when macOS is chosen in the row above, then the instruction stays open and shows the setup address', async () => {
      await wizard.tile('os-mac').click();
      await wizard.expectCursor('guide');
      await expect.soft(app.proxyCard.addressText).toHaveText(/^http:\/\/127\.0\.0\.1:\d+\/proxy\.pac$/);
    });

    await test.step('when Next is clicked, then the exit screen is the last one, step 5 of 5, and the exit mark is kept', async () => {
      await wizard.next();
      await wizard.expectScreen('exit');
      await expect.soft(wizard.progress).toHaveText('Step 5 of 5');
      await expect.poll(() => wizard.setupMark()).toBe('exit');
    });

    await test.step('when Finish is clicked, then Overview opens and the done mark is kept', async () => {
      await wizard.finish();
      await expect.soft(wizard.overviewTab).toHaveAttribute('aria-selected', 'true');
      await expect.soft(wizard.region).toHaveCount(0);
      await expect.soft(app.proxyCard.card).toBeVisible();
      await expect.poll(() => wizard.setupMark()).toBe('done');
    });
  });

  test('126-wizard-term-hint: Tab from the instruction heading reaches the term "PAC" and its hint shows; Escape hides it', async ({ app, page }) => {
    const wizard = app.wizard;

    await test.step('given: the instruction for all apps on macOS in Chrome; the hint is hidden', async () => {
      await wizard.open();
      await wizard.toGuide('system', 'mac');
      await expect.soft(wizard.tooltip).toBeHidden();
    });

    await test.step('when Tab is pressed from the heading, then the first term is focused and its hint is visible', async () => {
      await wizard.rowHeading('guide').focus();
      await page.keyboard.press('Tab');
      await expect.soft(wizard.terms.first()).toBeFocused();
      await expect.soft(wizard.terms.first()).toHaveText(/PAC/);
      await expect.soft(wizard.tooltip).toBeVisible();
    });

    await test.step('when Escape is pressed, then the term loses the focus and the hint hides', async () => {
      await page.keyboard.press('Escape');
      await expect.soft(wizard.terms.first()).not.toBeFocused();
      await expect.soft(wizard.tooltip).toBeHidden();
    });
  });

  test('127-wizard-back: Back on the exit screen returns to the page with the earlier choices pressed; the questions page has no Back', async ({ app }) => {
    const wizard = app.wizard;

    await test.step('given: Windows is chosen (not the default of the system in use), the instruction is open; no Back on the page', async () => {
      await wizard.open();
      await expect.soft(wizard.backButton, 'the questions page in a network').toHaveCount(0);
      await wizard.toGuide('system', 'windows');
      await expect.soft(wizard.backButton).toHaveCount(0);
    });

    await test.step('when Next and then Back are clicked, then the page is open down to the instruction with the choices pressed', async () => {
      await wizard.next();
      await wizard.expectScreen('exit');
      await wizard.back();
      await wizard.expectCursor('guide');
      await expect.soft(wizard.tile('os-windows')).toHaveAttribute('aria-pressed', 'true');
      await expect.soft(wizard.tile('os-mac')).toHaveAttribute('aria-pressed', 'false');
      await expect.soft(wizard.tile('scope-system')).toHaveAttribute('aria-pressed', 'true');
    });
  });
});

test.describe('layout of the wizard screens', () => {
  test.skip(({ targetKind }) => targetKind === 'node', 'the proxy is on in the demo node snapshot; on a real node it depends on startup flags');

  test('128-wizard-exit-spacing: on the exit screen the name hint is at least 8px above the "Allow other members" row', async ({ app, page }) => {
    const wizard = app.wizard;
    await wizard.open();
    await wizard.toGuide('system', 'mac');
    await wizard.next();
    await wizard.expectScreen('exit');
    const hint = page.locator('#wizard-screen').getByText('Others see you by this name');
    const row = page.locator('#wizard-screen .home-exit-toggle');
    await expect(row).toBeVisible();
    const [h, r] = [await hint.boundingBox(), await row.boundingBox()];
    expect(r!.y - (h!.y + h!.height)).toBeGreaterThanOrEqual(8);
  });

  test('130-wizard-strip-side: the strip of steps stands left of the page on a wide card and above it on a narrow one', async ({ app, page }) => {
    const wizard = app.wizard;
    await page.setViewportSize({ width: 1280, height: 900 });
    await wizard.open();
    await wizard.toGuide('system', 'mac');
    const [wide, wideRows] = [await wizard.strip.boundingBox(), await wizard.screen.boundingBox()];
    expect(wide!.x + wide!.width).toBeLessThanOrEqual(wideRows!.x);
    await page.setViewportSize({ width: 390, height: 900 });
    const [narrow, narrowRows] = [await wizard.strip.boundingBox(), await wizard.screen.boundingBox()];
    expect(narrow!.y + narrow!.height).toBeLessThanOrEqual(narrowRows!.y);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  });

  for (const width of [1280, 390]) {
    for (const term of ['first', 'last'] as const) {
      test(`129-wizard-term-tip-${width}-${term}: the hint of a term opens at the word and stays inside the window at ${width}px`, async ({ app, page }) => {
        const wizard = app.wizard;
        await page.setViewportSize({ width, height: 900 });
        await page.emulateMedia({ colorScheme: 'light' });
        await wizard.open();
        await wizard.toGuide('system', 'mac');
        await wizard.moreOptionsSummary.click();
        await app.proxyCard.manualToggle.click();
        const word = wizard.terms[term]();
        await word.focus();
        await expect(wizard.tooltip).toBeVisible();
        const [w, t] = [await word.boundingBox(), await wizard.tooltip.boundingBox()];
        expect(Math.abs(t!.x - w!.x)).toBeLessThanOrEqual(width === 1280 && 80 || 280);
        expect(t!.x + t!.width).toBeLessThanOrEqual(width);
        expect(t!.x).toBeGreaterThanOrEqual(0);
        expect(t!.y).toBeGreaterThanOrEqual(w!.y + w!.height - 2);
        expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
      });
    }
  }
});
