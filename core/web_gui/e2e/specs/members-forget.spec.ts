import { test, expect } from '../fixtures/index';
import { MembersSection } from '../pages/members-section';
import { MEMBERS_WORDS as W } from '../pages/glossary';

// The demo started with `--members-forget` adds four rows to the five of the plain demo: old-desktop (offline for 9 days),
// ghost-key (never seen online), lost-phone (offline for 2 days, peer-one already proposed removing it) and old-tablet
// (removed by you and peer-one). peer-one is online. A real node starts alone, so every case is for the demo only.

const DESKTOP = 'old-desktop';
const GHOST = 'ghost-key';
const PHONE = 'lost-phone';
const TABLET = 'old-tablet';
const ONE = 'peer-one';

test.describe('removing offline members (demo --members-forget)', () => {
  test.skip(({ targetKind }) => targetKind === 'node', 'the member list of the demo node is fixed; a real node starts alone');
  test.use({ targetFlags: { extraArgs: ['--members-forget'] } });

  test('97-members-stale-group: members offline for over a week are folded into one group; the rest stay in the list', async ({ app, page }) => {
    const members = new MembersSection(page);
    await app.tabs.open('peers');

    await test.step('the group counts two and says when each was last online', async () => {
      await expect(members.staleList.getByText(W.staleSummary(2))).toBeVisible();
      await members.staleList.locator('summary').click();
      await expect(members.groupRow(members.staleList, DESKTOP)).toContainText(W.lastOnline('9 d ago'));
      await expect(members.groupRow(members.staleList, GHOST)).toContainText(W.neverSeen);
    });

    await test.step('a member offline for two days stays in the main list', async () => {
      await expect(members.row(PHONE)).toBeVisible();
      await expect(members.row(DESKTOP)).toHaveCount(0);
      await expect(members.staleList).not.toContainText(PHONE);
    });
  });

  test('98-members-forget-propose: "⋯" → "Remove from network…" → "Propose removal": the row moves to the removed group and waits', async ({ app, page }) => {
    const members = new MembersSection(page);
    await app.tabs.open('peers');
    await members.staleList.locator('summary').click();

    await test.step('the dialog explains the removal', async () => {
      await members.pick(DESKTOP, W.menuForget);
      await expect(members.dialogTitle(W.forgetTitle(DESKTOP))).toBeVisible();
    });

    await test.step('"Propose removal": the row is in the removed group with "Waiting…", the status says what to do', async () => {
      await members.dialogButton(W.forgetPropose).click();
      await expect(members.forgottenList.getByText(W.forgottenSummary(2))).toBeVisible();
      await members.forgottenList.locator('summary').click();
      await expect(members.groupRow(members.forgottenList, DESKTOP)).toContainText(W.forgetWaiting);
      await expect(members.statusText(W.forgetProposedStatus(DESKTOP))).toBeVisible();
      await expect(members.staleList.getByText(W.staleSummary(1))).toBeVisible();
    });
  });

  test('99-members-forget-confirm: a removal that peer-one proposed: "Confirm removal…" removes the member for everyone', async ({ app, page }) => {
    const members = new MembersSection(page);
    await app.tabs.open('peers');

    await test.step('the row carries the badge and the confirming menu item', async () => {
      await expect(members.badge(PHONE, W.badgeForgetProposed)).toBeVisible();
      await members.openMenu(PHONE);
      await expect(members.menuItem(W.menuConfirmForget)).toBeVisible();
      await expect(members.menuItem(W.menuForget)).toHaveCount(0);
    });

    await test.step('the second vote removes it: the dialog, the group, the status', async () => {
      await members.menuItem(W.menuConfirmForget).click();
      await expect(members.dialogTitle(W.forgetConfirmTitle(PHONE))).toBeVisible();
      await members.dialogButton(W.forgetAction(PHONE)).click();
      await expect(members.row(PHONE)).toHaveCount(0);
      await expect(members.forgottenList).toContainText(PHONE);
      await expect(members.statusText(W.forgottenStatus(PHONE))).toBeVisible();
    });
  });

  test('100-members-forget-undo: "Undo removal" on a removed member returns it to the list', async ({ app, page }) => {
    const members = new MembersSection(page);
    await app.tabs.open('peers');
    await members.forgottenList.locator('summary').click();
    await expect(members.groupRow(members.forgottenList, TABLET)).toBeVisible();

    await members.pick(TABLET, W.menuUndoForget);

    await test.step('the member is out of the removed group and the status says so', async () => {
      await expect(members.forgottenList).toHaveCount(0);
      await expect(members.statusText(W.unforgottenStatus(TABLET))).toBeVisible();
    });

    await test.step('it is offline for over a week, so it is back in that group', async () => {
      await expect(members.staleList).toContainText(TABLET);
    });
  });

  test('101-members-ban-word: a member who is online can only be banned; the removal item is for offline members', async ({ app, page }) => {
    const members = new MembersSection(page);
    await app.tabs.open('peers');

    await members.openMenu(ONE);
    await expect(members.menuItem(W.menuBan)).toBeEnabled();
    await expect(members.menuItem(W.menuForget)).toHaveCount(0);
    await expect(members.menuItem(W.menuConfirmForget)).toHaveCount(0);
  });
});

test.describe('this computer removed from the list (demo --me-removed)', () => {
  test.skip(({ targetKind }) => targetKind === 'node', 'the member list of the demo node is fixed; a real node starts alone');

  test.describe('one network', () => {
    test.use({ targetFlags: { extraArgs: ['--members-forget', '--me-removed'] } });

    test('102-members-me-removed: a card with a way to join again and a grey line in the header', async ({ app, page }) => {
      const members = new MembersSection(page);
      await app.tabs.open('peers');

      await expect(members.forgottenCard).toBeVisible();
      await expect(members.forgottenCard.getByRole('button', { name: W.forgottenJoin })).toBeVisible();
      await expect(app.topPanel.statusLine).toContainText(W.forgottenStatusLine);
      await expect(page.getByRole('button', { name: /Remove all/ })).toHaveCount(0);
    });
  });

  test.describe('two networks', () => {
    test.use({ targetFlags: { extraArgs: ['--members-forget', '--me-removed', '--nets', '2'] } });

    test('103-members-removed-per-tab: the card is on the tab of the network that removed this computer, the other tab has the list', async ({
      app,
      page,
    }) => {
      const members = new MembersSection(page);
      await app.tabs.open('peers');
      const slots = page.locator('#peers-net-tabs a.ntab[data-slot]');

      await test.step('the first tab (main) has the card', async () => {
        await expect(slots).toHaveCount(2);
        await expect(members.forgottenCard).toBeVisible();
      });

      await test.step('the second tab has the member list and no card', async () => {
        await slots.nth(1).click();
        await expect(members.forgottenCard).toHaveCount(0);
        await expect(members.list).toBeVisible();
        await expect(page.getByRole('button', { name: /Remove all/ })).toHaveCount(0);
      });
    });
  });
});
