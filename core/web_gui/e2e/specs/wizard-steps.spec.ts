import { test, expect } from '../fixtures/index';
import { WIZARD_TEXT } from '../pages/home/wizard';

// The step-by-step setup of a node that is already in a network: one screen at a time.
// The proxy is on in the demo snapshot; on a real node it depends on startup flags.

test.describe('the setup wizard of a node in a network', () => {
  test.skip(({ targetKind }) => targetKind === 'node', 'the proxy is on in the demo node snapshot; on a real node it depends on startup flags');

  test('125-wizard-steps: all apps → macOS → Chrome → instruction → exit → Finish: the progress counts 2 to 6 of 6, the instruction shows the address, Finish opens Overview and keeps the done mark', async ({ app }) => {
    const wizard = app.wizard;

    await test.step('given: a node in a network, the Setup tab is open — the first question is about the scope, step 2 of 6', async () => {
      await wizard.open();
      await wizard.expectStep('scope');
      await expect.soft(wizard.question).toHaveText(WIZARD_TEXT.scopeQuestion);
      await expect.soft(wizard.progress).toHaveText('Step 2 of 6');
    });

    await test.step('when "All apps" is chosen, then the system question follows at once, step 3 of 6', async () => {
      await wizard.tile('scope-system').click();
      await wizard.expectStep('os');
      await expect.soft(wizard.question).toHaveText(WIZARD_TEXT.osQuestion);
      await expect.soft(wizard.progress).toHaveText('Step 3 of 6');
    });

    await test.step('when macOS is chosen, then the browser question follows, step 4 of 6', async () => {
      await wizard.tile('os-mac').click();
      await wizard.expectStep('browser');
      await expect.soft(wizard.question).toHaveText(WIZARD_TEXT.browserQuestion);
      await expect.soft(wizard.progress).toHaveText('Step 4 of 6');
    });

    await test.step('when Chrome is chosen, then the instruction shows the setup address, step 5 of 6', async () => {
      await wizard.tile('browser-chrome').click();
      await wizard.expectStep('guide');
      await expect.soft(wizard.progress).toHaveText('Step 5 of 6');
      await expect.soft(app.proxyCard.addressText).toHaveText(/^http:\/\/127\.0\.0\.1:\d+\/proxy\.pac$/);
    });

    await test.step('when Next is clicked, then the exit screen is the last one, step 6 of 6, and the exit mark is kept', async () => {
      await wizard.next();
      await wizard.expectStep('exit');
      await expect.soft(wizard.progress).toHaveText('Step 6 of 6');
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

  test('126-wizard-term-hint: Tab from the question reaches the term "PAC" and its hint shows; Escape hides it', async ({ app, page }) => {
    const wizard = app.wizard;

    await test.step('given: the instruction for all apps on macOS in Chrome; the hint is hidden', async () => {
      await wizard.open();
      await wizard.toGuide('system', 'mac', 'chrome');
      await expect.soft(wizard.tooltip).toBeHidden();
    });

    await test.step('when Tab is pressed from the question, then the first term is focused and its hint is visible', async () => {
      await wizard.question.focus();
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

  test('127-wizard-back: Back goes through the steps and shows the earlier choice pressed; the first screen has no Back', async ({ app }) => {
    const wizard = app.wizard;

    await test.step('given: Windows and Edge are chosen (not the defaults of the browser in use), the instruction is open', async () => {
      await wizard.open();
      await expect.soft(wizard.backButton, 'the first screen in a network').toHaveCount(0);
      await wizard.toGuide('system', 'windows', 'edge');
    });

    await test.step('when Back is clicked, then the browser question shows Edge pressed and the others not', async () => {
      await wizard.back();
      await wizard.expectStep('browser');
      await expect.soft(wizard.tile('browser-edge')).toHaveAttribute('aria-pressed', 'true');
      await expect.soft(wizard.tile('browser-chrome')).toHaveAttribute('aria-pressed', 'false');
    });

    await test.step('when Back is clicked again, then the system question shows Windows pressed', async () => {
      await wizard.back();
      await wizard.expectStep('os');
      await expect.soft(wizard.tile('os-windows')).toHaveAttribute('aria-pressed', 'true');
      await expect.soft(wizard.tile('os-mac')).toHaveAttribute('aria-pressed', 'false');
    });

    await test.step('when Back is clicked once more, then the scope question shows "All apps" pressed and no Back', async () => {
      await wizard.back();
      await wizard.expectStep('scope');
      await expect.soft(wizard.tile('scope-system')).toHaveAttribute('aria-pressed', 'true');
      await expect.soft(wizard.tile('scope-browser')).toHaveAttribute('aria-pressed', 'false');
      await expect.soft(wizard.backButton).toHaveCount(0);
    });
  });
});
