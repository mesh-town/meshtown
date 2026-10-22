import { test, expect } from '../fixtures/index';
import { GLOSSARY } from '../pages/glossary';

test.describe('--cli-override node.name', () => {
  test.use({ targetFlags: { extraArgs: ['--cli-override', 'node.name'] } });

  test('2-cli-override: --cli-override node.name: shown as set at startup (rust — the first screen, node — the form)', async ({
    app,
  }) => {
    await app.tabs.open('settings');

    await test.step('the field is shown as set at startup', async () => {
      const phrase = GLOSSARY.cliOverrideNote;
      // The phrase may be on the first screen or in Settings: look at both sections.
      await expect
        .poll(async () => {
          const home = (await app.homePanel.root.textContent()) ?? '';
          const settings = (await app.settings.root.textContent()) ?? '';
          return `${home}\n${settings}`.includes(phrase);
        })
        .toBe(true);
    });
  });
});

test.describe('--file-state readonly', () => {
  test.use({ targetFlags: { extraArgs: ['--file-state', 'readonly'] } });

  test('3-readonly: --file-state readonly: saving is unavailable up front', async ({ app }) => {
    await app.tabs.open('settings');

    await test.step('the "Save" button is disabled up front', async () => {
      await expect.soft(app.settings.saveButton, 'the save button is enabled').toBeDisabled();
    });

    await test.step('the reason is shown next to it', async () => {
      await expect.soft(app.settings.writableReason, 'the reason text is empty').toContainText(/\S/);
    });
  });
});
