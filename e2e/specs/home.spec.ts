import { test, expect } from '../fixtures/index';
import { GLOSSARY } from '../pages/glossary';
import { SECTIONS } from '../pages/tabs-bar';

const VIEWPORT = { width: 1280, height: 800 };
const TAB_LABELS = ['Home', /^Members\d*$/, 'Site checks', 'Ports', 'Diagnostics', 'Log', 'Settings'];

test('19-home-first-screen: a newcomer\'s first screen fits in 800px without "Advanced" and without #nav', async ({
  app,
  page,
  targetKind,
}) => {
  await app.resize(VIEWPORT);
  // On the public directory a real node shows the line "Loading network data…" for the first 60 s once other members are known
  // (+1 line in "Network invite"; it is never shown while only you are in the network); the measurement is the settled
  // layout, so the transitional line is hidden.
  await page.locator('#home-net-status').evaluate((el) => ((el as HTMLElement).hidden = true));

  await test.step('the bottom of the exit, name and members fields and of the browser card intro line fits in 800px', async () => {
    const fields = {
      'exit (#field-exit-enabled)': app.nameCard.exitToggle,
      'name (#field-node-name)': app.nameCard.nameField,
      '#home-exit': app.exitCard.card,
      '#proxy-hint (browser card intro line)': app.proxyCard.hint,
    };
    for (const [name, loc] of Object.entries(fields)) {
      await expect.soft(loc, `element not in the DOM: ${name}`).toHaveCount(1);
      expect.soft(await app.homePanel.bottom(loc), `bottom of ${name}`).toBeLessThanOrEqual(VIEWPORT.height);
    }
  });

  await test.step('the "Exit via…" card explains the word "exit" in one visible phrase', async () => {
    await expect(app.exitCard.card.getByText(GLOSSARY.exitExplain)).toBeVisible();
  });

  await test.step('"Home" is active by default: #sec-home is visible, six panels are hidden', async () => {
    await expect.soft(app.tabs.panel('home')).toBeVisible();
    for (const section of SECTIONS.filter((s) => s !== 'home')) {
      // A panel must be in the DOM (hidden): a missing one does not count as "hidden".
      await expect.soft(app.tabs.panel(section), `panel ${section} is not in the DOM`).toHaveCount(1);
      await expect.soft(app.tabs.panel(section), `panel ${section}`).toBeHidden();
    }
  });

  await test.step('#tabs: seven tabs, labels verbatim in order', async () => {
    await expect.soft(app.homePanel.tabList).toHaveCount(1);
    await expect.soft(app.homePanel.tabItems).toHaveText(TAB_LABELS);
  });

  await app.homePanel.expectTargetSpecific(targetKind, app.tabs.panel('home'));
});

test('76-home-pac-card: the browser card — the PAC auto-config address is visible, the "Your system" group has one pressed button, manual setup is collapsed and expands', async ({
  app,
  targetKind,
}) => {
  test.skip(targetKind === 'node', 'the proxy is on in the demo node snapshot; on a real node it depends on startup flags');
  const card = app.proxyCard;

  await test.step('an address like http://127.0.0.1:<port>/proxy.pac is visible, with "Copy address" next to it', async () => {
    await expect.soft(card.card).toHaveCount(1);
    await expect.soft(card.addressText).toBeVisible();
    await expect.soft(card.addressText).toHaveText(/^http:\/\/127\.0\.0\.1:\d+\/proxy\.pac$/);
    await expect.soft(card.copyButton).toHaveText('Copy address');
  });

  await test.step('the "Your system" group: four buttons, exactly one pressed', async () => {
    await expect.soft(card.osGroup).toBeVisible();
    await expect.soft(card.osButtons).toHaveCount(4);
    await expect.soft(card.osPressed).toHaveCount(1);
  });

  await test.step('the manual block is collapsed; "Not working? Set up manually ▸" expands it, with "Copy port" inside', async () => {
    await expect.soft(card.manualSetup).toHaveCount(0);
    await card.manualToggle.click();
    await expect.soft(card.manualToggle).toHaveAttribute('aria-expanded', 'true');
    await expect.soft(card.manualToggle).toHaveText(/▾/);
    await expect.soft(card.manualSetup).toBeVisible();
    await expect.soft(card.copyPortButton).toBeVisible();
  });
});
