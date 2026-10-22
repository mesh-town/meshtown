import { test, expect } from '../fixtures/index';
import { MembersSection } from '../pages/members-section';
import { MEMBERS_WORDS as W } from '../pages/glossary';

// The demo node serves five members: you, peer-one (ordinary), peer-two (peer-one proposed removing it),
// peer-three (already removed) and stray-laptop (not in the list). The page of a real node has none of them.

const ONE = 'peer-one';
const TWO = 'peer-two';
const REMOVED = 'peer-three';

test('79-members-list: the "Members" tab: the other rows have the "⋯" menu; someone else\'s proposal shows the banner and the counter', async ({
  app,
  page,
  targetKind,
}) => {
  test.skip(targetKind === 'node', 'the member list of the demo node is fixed; a real node starts alone');
  const members = new MembersSection(page);
  await app.tabs.open('peers');

  await test.step('other members have the "⋯" menu; the removed one is not in the list', async () => {
    await expect(members.menuButton(ONE)).toBeVisible();
    await expect(members.menuButton(TWO)).toBeVisible();
    await expect(members.row(REMOVED)).toHaveCount(0);
  });

  await test.step('the proposal of another member: banner, label, line, counter on the tab', async () => {
    await expect(members.banner).toBeVisible();
    await expect(members.banner).toContainText(W.voteTitle(ONE, TWO));
    await expect(members.banner.getByRole('button', { name: W.confirmRemoval })).toBeVisible();
    await expect(members.badge(TWO, W.badgeProposed)).toBeVisible();
    await expect(members.row(TWO)).toContainText(W.proposedByLine);
    await expect(members.tabChip()).toHaveText('1');
  });

  await test.step('"Not now" hides the banner; the label stays', async () => {
    await members.banner.getByRole('button', { name: W.notNow }).click();
    await expect(members.banner).toHaveCount(0);
    await expect(members.badge(TWO, W.badgeProposed)).toBeVisible();
  });

  await test.step('the group of former members is folded and counts one', async () => {
    await expect(members.removedList.getByText(W.removedSummary(1))).toBeVisible();
    await expect(members.removedList).toContainText(REMOVED);
  });
});

test('80-members-exclude: proposing a removal: "⋯" → "Remove from network…" → dialog (Esc returns the focus to "⋯") → "Propose removal" → the label and the status', async ({
  app,
  page,
  targetKind,
}) => {
  test.skip(targetKind === 'node', 'the member list of the demo node is fixed; a real node starts alone');
  const members = new MembersSection(page);
  await app.tabs.open('peers');

  await test.step('the menu of a regular member: block and remove', async () => {
    await members.openMenu(ONE);
    await expect(members.menuItem(W.menuBlock)).toBeVisible();
    await expect(members.menuItem(W.menuRemove)).toBeEnabled();
  });

  await test.step('the dialog asks about the removal; Esc closes it and the focus returns to "⋯"', async () => {
    await members.menuItem(W.menuRemove).click();
    await expect(members.dialogTitle(W.removeTitle(ONE))).toBeVisible();
    await expect(members.dialogButton(W.dialogCancel)).toBeFocused();
    await page.keyboard.press('Escape');
    await expect(members.dialog).toBeHidden();
    await expect(members.menuButton(ONE)).toBeFocused();
    await expect(members.badge(ONE, W.badgeProposed)).toHaveCount(0);
  });

  await test.step('"Propose removal": the row is labelled, the status explains what to do next', async () => {
    await members.pick(ONE, W.menuRemove);
    await members.dialogButton(W.proposeRemoval).click();
    await expect(members.badge(ONE, W.badgeProposed)).toBeVisible();
    await expect(members.row(ONE)).toContainText(W.waitingLine);
    await expect(members.statusText(W.proposedStatus(ONE))).toBeVisible();
    await members.openMenu(ONE);
    await expect(members.menuItem(W.menuWithdraw)).toBeEnabled();
    await expect(members.menuItem(W.menuWaiting)).toHaveCount(0);
  });
});

test.describe('--members-second-vote', () => {
  test.use({ targetFlags: { extraArgs: ['--members-second-vote'] } });

  test('81-members-removed: a second member agrees at once: the row moves to "No longer in the network"', async ({
    app,
    page,
    targetKind,
  }) => {
    test.skip(targetKind === 'node', 'the second vote is a flag of the demo node');
    const members = new MembersSection(page);
    await app.tabs.open('peers');
    await expect(members.removedList.getByText(W.removedSummary(1))).toBeVisible();

    await members.pick(ONE, W.menuRemove);
    await members.dialogButton(W.proposeRemoval).click();

    await expect(members.row(ONE)).toHaveCount(0);
    // Two members are left, so one vote is enough from now on and the waiting proposal for peer-two passes too.
    await expect(members.removedList.getByText(W.removedSummary(3))).toBeVisible();
    await expect(members.removedList).toContainText(ONE);
    await expect(members.statusText(W.removedStatus(ONE))).toBeVisible();
  });
});

test('82-members-block: blocking on this computer needs no vote: label, "Unblock", status', async ({ app, page, targetKind }) => {
  test.skip(targetKind === 'node', 'the member list of the demo node is fixed; a real node starts alone');
  const members = new MembersSection(page);
  await app.tabs.open('peers');

  await test.step('"⋯" → "Block on this computer…" → the dialog → "Block peer-one"', async () => {
    await members.pick(ONE, W.menuBlock);
    await expect(members.dialogTitle(W.blockTitle(ONE))).toBeVisible();
    await members.dialogButton(W.blockAction(ONE)).click();
    await expect(members.badge(ONE, W.badgeBlocked)).toBeVisible();
    await expect(members.statusText(W.blockedStatus(ONE))).toBeVisible();
    await expect(members.badge(ONE, W.badgeProposed)).toHaveCount(0);
  });

  await test.step('"Unblock" takes the label off at once, without a dialog', async () => {
    await members.row(ONE).getByRole('button', { name: `${W.unblock} ${ONE}` }).click();
    await expect(members.badge(ONE, W.badgeBlocked)).toHaveCount(0);
    await expect(members.dialog).toBeHidden();
  });
});

test.describe('--me-excluded', () => {
  test.use({ targetFlags: { extraArgs: ['--me-excluded'] } });

  test('83-members-me-excluded: a removed computer sees the card with the way back instead of the list', async ({
    app,
    page,
    targetKind,
  }) => {
    test.skip(targetKind === 'node', 'starting removed is a flag of the demo node');
    const members = new MembersSection(page);
    await app.tabs.open('peers');
    await expect(members.excludedCard).toBeVisible();
    await expect(members.excludedCard.getByRole('button', { name: W.excludedJoin })).toBeVisible();
    await expect(members.list).toHaveCount(0);
  });
});

test('84-members-withdraw: withdrawing my own proposal: "⋯" → "Withdraw proposal" (no dialog) → the label goes and the status says so', async ({
  app,
  page,
  targetKind,
}) => {
  test.skip(targetKind === 'node', 'the member list of the demo node is fixed; a real node starts alone');
  const members = new MembersSection(page);
  await app.tabs.open('peers');
  await members.pick(ONE, W.menuRemove);
  await members.dialogButton(W.proposeRemoval).click();
  await expect(members.badge(ONE, W.badgeProposed)).toBeVisible();

  await members.pick(ONE, W.menuWithdraw);

  await expect(members.badge(ONE, W.badgeProposed)).toHaveCount(0);
  await expect(members.row(ONE)).not.toContainText(W.waitingLine);
  await expect(members.dialog).toBeHidden();
  await expect(members.statusText(W.withdrawnStatus)).toBeVisible();
});

test('85-members-reason: the removal dialog has an optional reason field of at most 80 characters, empty at every opening', async ({
  app,
  page,
  targetKind,
}) => {
  test.skip(targetKind === 'node', 'the member list of the demo node is fixed; a real node starts alone');
  const members = new MembersSection(page);
  await app.tabs.open('peers');
  await members.pick(ONE, W.menuRemove);

  await test.step('typing 90 characters leaves exactly 80 in the field', async () => {
    await expect(members.reasonField()).toBeVisible();
    await members.reasonField().pressSequentially('a'.repeat(90));
    await expect(members.reasonField()).toHaveValue('a'.repeat(80));
  });

  await test.step('Cancel and open again: the field is empty', async () => {
    await members.dialogButton(W.dialogCancel).click();
    await members.pick(ONE, W.menuRemove);
    await expect(members.reasonField()).toHaveValue('');
  });

  await test.step('a reason does not change the outcome: the proposal is made', async () => {
    await members.reasonField().pressSequentially('lost laptop');
    await members.dialogButton(W.proposeRemoval).click();
    await expect(members.badge(ONE, W.badgeProposed)).toBeVisible();
  });
});

test('86-members-reason-shown: the proposal of another member shows its reason in the banner and in the confirmation dialog', async ({
  app,
  page,
  targetKind,
}) => {
  test.skip(targetKind === 'node', 'the member list of the demo node is fixed; a real node starts alone');
  const members = new MembersSection(page);
  await app.tabs.open('peers');
  await expect(members.banner).toContainText(W.reasonFrom(ONE, 'lost laptop'));

  await members.banner.getByRole('button', { name: W.confirmRemoval }).click();

  await expect(members.dialog).toContainText(W.reasonFrom(ONE, 'lost laptop'));
  await members.dialogButton(W.dialogCancel).click();
  await expect(members.dialog).toBeHidden();
});

test('87-members-leave-settings: "Leave network…" in Settings → the dialog → the card "You left the network"', async ({ app, page, targetKind }) => {
  test.skip(targetKind === 'node', 'leaving is checked on the demo node, whose member list is fixed');
  const members = new MembersSection(page);
  await app.tabs.open('settings');

  await test.step('the button asks first', async () => {
    await members.settingsLeaveButton().click();
    await expect(members.dialogTitle(W.leaveTitle)).toBeVisible();
    await expect(members.dialogButton(W.dialogCancel)).toBeFocused();
  });

  await test.step('confirming: the line under the button says so and the button goes', async () => {
    await members.dialogButton(W.leaveAction).click();
    await expect(members.statusText(W.leftStatus)).toBeVisible();
    await expect(members.settingsLeaveButton()).toHaveCount(0);
  });

  await test.step('the "Members" tab shows the card with the way back instead of the list', async () => {
    await app.tabs.open('peers');
    await expect(members.leftCard).toBeVisible();
    await expect(members.list).toHaveCount(0);
  });
});

const ANNA = 'anna-laptop';

test.describe('--members-pending', () => {
  test.use({ targetFlags: { extraArgs: ['--members-pending'] } });

  test('88-members-admit: a pending node, confirm, row becomes a member', async ({ app, page, targetKind }) => {
    test.skip(targetKind === 'node', 'the waiting node is a flag of the demo node');
    const members = new MembersSection(page);
    await app.tabs.open('peers');

    await test.step('the request is listed apart with the label, the count and the "Confirm…" button; the tab counts it', async () => {
      await expect(members.pendingList).toContainText(W.pendingTitle);
      await expect(members.pendingRow(ANNA)).toContainText(W.badgePending);
      await expect(members.pendingRow(ANNA)).toContainText(W.admissionLine);
      await expect(members.pendingRow(ANNA)).toContainText(W.labelLine);
      await expect(members.row(ANNA)).toHaveCount(0);
      await expect(members.tabChip(2)).toBeVisible();
    });

    await test.step('"Confirm…" opens the dialog; Cancel changes nothing', async () => {
      await members.pendingRow(ANNA).getByRole('button', { name: W.admitAria(ANNA) }).click();
      await expect(members.dialogTitle(W.admitTitle(ANNA))).toBeVisible();
      await members.dialogButton(W.dialogCancel).click();
      await expect(members.dialog).toBeHidden();
      await expect(members.pendingRow(ANNA)).toBeVisible();
    });

    await test.step('confirming: the confirmation admits the node, the row moves to the list', async () => {
      await members.pendingRow(ANNA).getByRole('button', { name: W.admitAria(ANNA) }).click();
      await members.confirmYes().click();
      await expect(members.pendingList).toHaveCount(0);
      await expect(members.row(ANNA)).toBeVisible();
      await expect(members.statusText(W.admittedStatus(ANNA))).toBeVisible();
      await expect(members.tabChip()).toBeVisible();
    });
  });
});

test.describe('--me-pending', () => {
  test.use({ targetFlags: { extraArgs: ['--me-pending'] } });

  test('89-members-me-pending: a newcomer sees the waiting card', async ({ app, page, targetKind }) => {
    test.skip(targetKind === 'node', 'starting as a waiting node is a flag of the demo node');
    const members = new MembersSection(page);
    await app.tabs.open('peers');
    await expect(members.pendingCard).toBeVisible();
    await expect(members.pendingCard).toContainText(W.pendingTitle);
    await expect(members.pendingCard).toContainText(W.meWaiting);
  });
});
