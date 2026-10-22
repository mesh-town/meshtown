import { test, expect } from '../fixtures/index';
import { GLOSSARY, peerLabel } from '../pages/glossary';
import { POLL_TIMEOUT_MS, expectConfigExits, type StateDoc, type Peer } from '../helpers/route-exits-api';
import { HomeSection } from '../pages/home/section';
import { TabsBar } from '../pages/tabs-bar';

const NET_TITLE = 'webcheck-created-net';
const EXPECTED_ALONE = 'Only you in "' + NET_TITLE + '" so far. Invite the people you trust.';
const EXPECTED_HINT = GLOSSARY.exitListHint;

test.describe('27-home-create-network', () => {
  test.use({ targetFlags: { extraArgs: ['--no-network'] } });

  test('27-home-create-network: "Create network": the form collapses, the join field is clean, the join card moves into the "Network" card on "Settings", the invite card says "Only you in …" and keeps #invite-status empty', async ({ app, page }) => {
    const net = app.netCard;
    await net.createNetwork(NET_TITLE);

    await test.step('#create-box is collapsed, #join-key-input and #join-key-error are empty after "Create network"', async () => {
      await expect.soft(net.createLabelInput).toBeHidden({ timeout: 6000 });
      await expect.soft(app.joinCard.keyInput).toHaveValue('');
      await expect.soft(app.joinCard.keyError).toBeHidden();
    });

    await test.step('the join card moved to "Settings" (#net-join-slot) and left "Home"', async () => {
      await expect.soft(app.joinCard.netSlotKeyInput).toHaveCount(1);
      await expect.soft(app.joinCard.inHomeSlot).toHaveCount(0);
    });

    await test.step('on "Home" the setup is on step 2: "✓ You joined the network", "Set up your browser"', async () => {
      await expect.soft(app.homePanel.wizard).toContainText(GLOSSARY.wizardJoined);
      await expect.soft(app.homePanel.wizard.getByRole('heading', { name: /Set up your browser/ })).toBeVisible();
    });

    await test.step('the invite card is under the setup — #netkey-invite-btn is visible, #invite-status is empty (no "Click Invite a member" notice)', async () => {
      // The button depends on state.network — it arrives with the next poll tick, so wait for it separately.
      await expect.soft(net.inviteButton).toBeVisible({ timeout: 8000 });
      await expect.soft(net.inviteStatus).toBeHidden();
      await expect.soft(net.inviteStatus).toHaveText('');
    });

    await test.step('the counter says "Only you in …", the network status line (#home-net-status) is hidden: no "Loading network data…"', async () => {
      await expect.soft(page.locator('#home-members-count')).toHaveText(EXPECTED_ALONE, { timeout: 8000 });
      await expect.soft(page.locator('#home-net-status')).toBeHidden();
    });

  });
});

test('28-tabs-hash: the address "…#settings" activates the tab, "back" in history returns the previous one, #tabs sticks on scroll', async ({ page, target, consoleLog }) => {
  void consoleLog; // console listeners — before goto
  const home = new HomeSection(page);
  const tabs = new TabsBar(page);
  await home.gotoAndWait(`${target.origin}/?token=${target.token}#settings`);

  await test.step('the address "…#settings" activates the "Settings" tab', async () => {
    await expect.soft(tabs.tab('settings')).toHaveAttribute('aria-selected', 'true', { timeout: 4000 });
  });

  await tabs.click('log');

  await test.step('clicking the "Log" tab changes the address to "…#log"', async () => {
    await expect.soft(tabs.tab('log')).toHaveAttribute('aria-selected', 'true', { timeout: 4000 });
    expect.soft(await home.hash()).toBe('#log');
  });

  await home.back();

  await test.step('"back" in history returns the "Settings" tab', async () => {
    await expect.soft(tabs.tab('settings')).toHaveAttribute('aria-selected', 'true', { timeout: 4000 });
  });

  await test.step('#tabs stays within the window after scrolling down (sticky)', async () => {
    const { top, viewportHeight } = await home.tabsTopAfterScroll(tabs.root);
    expect.soft(top, `top=${top}`).toBeGreaterThanOrEqual(0);
    expect.soft(top, `window height ${viewportHeight}`).toBeLessThanOrEqual(viewportHeight);
  });
});

test.describe('29-exit-list-single-choice', () => {
  test.skip(({ targetKind }) => targetKind === 'node', 'a single node, no other members (like 17-peer-exit)');

  test('29-exit-list-single-choice: #exit-list: each option on its own row, clicking a node makes it the only chosen one, a hint about one node (single choice)', async ({ app, api }) => {
    const exit = app.exitCard;
    const state = await api.getJson<StateDoc>('/api/state');
    const peers = (state.body?.peers ?? []).filter((p) => !p.self);
    // Canned web_demo members with exit=true are needed so there are two candidates for the A -> B click.
    test.skip(peers.length < 2, `fewer than two demo members on the target (peers=${peers.length})`);
    const [a, b] = peers as [Peer, Peer];

    await test.step('each option on its own row (the row top grows from top to bottom)', async () => {
      await expect.soft(exit.list).toHaveCount(1);
      await expect.soft(exit.exitRows.nth(2), 'fewer than three rows').toBeVisible();
      const tops = await exit.rowTops();
      expect.soft(tops.length, `tops=${tops}`).toBeGreaterThanOrEqual(3);
      const sorted = tops.every((t, i) => i === 0 || (tops[i - 1] as number) < t);
      expect.soft(sorted, `tops=${tops}`).toBe(true);
    });

    await test.step('clicking a node makes it the only chosen one (route.exits == [B], not [A, B])', async () => {
      await exit.choice(peerLabel(a, false, state.body?.peers ?? [])).click();
      await expectConfigExits(api, [a.id_full]);
      await exit.choice(peerLabel(b, false, state.body?.peers ?? [])).click();
      await expectConfigExits(api, [b.id_full]);
    });

    await test.step('exactly one node radio button is checked — node B', async () => {
      await expect.soft(exit.choice(peerLabel(b, false, state.body?.peers ?? []))).toBeChecked({ timeout: POLL_TIMEOUT_MS });
      expect.soft(await exit.checkedCount()).toBe(1);
    });

    await test.step('row texts without "primary"/"backup"/"№" (single choice)', async () => {
      await expect.soft(exit.candidateRows.first(), 'no candidate rows').toBeVisible();
      const bad = (await exit.candidateRows.allTextContents()).filter((t) => /primary|backup|№/.test(t));
      expect.soft(bad, `texts with forbidden words: ${bad}`).toEqual([]);
    });

    await test.step('the hint under the list is the verbatim text about the one chosen member', async () => {
      await expect.soft(exit.exitHint).toBeVisible();
      await expect.soft(exit.exitHint).toHaveText(EXPECTED_HINT);
    });
  });
});
