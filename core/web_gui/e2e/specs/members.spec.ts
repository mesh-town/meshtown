import { test, expect } from '../fixtures/index';
import type { MeshApi } from '../fixtures/api';
import { MembersSection } from '../pages/members-section';
import { MEMBERS_WORDS as W } from '../pages/glossary';

// The demo node serves five members: you (the only admin), peer-one and peer-two (ordinary members), peer-three (already
// removed) and stray-laptop (not in the list). The page of a real node has none of them.

const ONE = 'peer-one';
const TWO = 'peer-two';
const EXCLUDED = 'peer-three';

interface StateDoc {
  network?: { label?: string | null } | null;
}

/** The name of the network the demo node serves, as the node reports it. */
async function netLabel(api: MeshApi): Promise<string> {
  const state = await api.getJson<StateDoc>('/api/state');
  const label = state.body?.network?.label;
  expect(label, 'the demo network has a name').toBeTruthy();
  return label as string;
}

test('79-members-list: the "Members" tab: the other rows have the "⋯" menu; the removed member is in the folded group', async ({
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
    await expect(members.row(EXCLUDED)).toHaveCount(0);
  });

  await test.step('the group of former members is folded and counts one', async () => {
    await expect(members.excludedList.getByText(W.excludedSummary(1))).toBeVisible();
    await expect(members.excludedList).toContainText(EXCLUDED);
  });
});

test('80-members-remove: "⋯" → "Remove from network…" → dialog (Esc returns the focus to "⋯") → "Remove" → the row moves to former members and the status says so', async ({
  app,
  page,
  api,
  targetKind,
}) => {
  test.skip(targetKind === 'node', 'the member list of the demo node is fixed; a real node starts alone');
  const members = new MembersSection(page);
  const net = await netLabel(api);
  await app.tabs.open('peers');

  await test.step('the menu of a regular member: block and remove', async () => {
    await members.openMenu(ONE);
    await expect(members.menuItem(W.menuBlock)).toBeVisible();
    await expect(members.menuItem(W.menuRemove)).toBeEnabled();
  });

  await test.step('the dialog asks about the removal; Esc closes it and the focus returns to "⋯"', async () => {
    await members.menuItem(W.menuRemove).click();
    await expect(members.dialogTitle(W.removeTitle(ONE, net))).toBeVisible();
    await expect(members.dialogButton(W.dialogCancel)).toBeFocused();
    await page.keyboard.press('Escape');
    await expect(members.dialog).toBeHidden();
    await expect(members.menuButton(ONE)).toBeFocused();
    await expect(members.row(ONE)).toHaveCount(1);
  });

  await test.step('"Remove": at once, with no second member needed', async () => {
    await members.pick(ONE, W.menuRemove);
    await members.dialogButton(W.removeAction).click();
    await expect(members.row(ONE)).toHaveCount(0);
    await expect(members.excludedList.getByText(W.excludedSummary(2))).toBeVisible();
    await expect(members.excludedList).toContainText(ONE);
    await expect(members.statusText(W.removedStatus(ONE))).toBeVisible();
  });
});

test('82-members-block: blocking on this computer is local: label, "Unblock", status', async ({ app, page, targetKind }) => {
  test.skip(targetKind === 'node', 'the member list of the demo node is fixed; a real node starts alone');
  const members = new MembersSection(page);
  await app.tabs.open('peers');

  await test.step('"⋯" → "Block on this computer…" → the dialog → "Block peer-one"', async () => {
    await members.pick(ONE, W.menuBlock);
    await expect(members.dialogTitle(W.blockTitle(ONE))).toBeVisible();
    await members.dialogButton(W.blockAction(ONE)).click();
    await expect(members.badge(ONE, W.badgeBlocked)).toBeVisible();
    await expect(members.statusText(W.blockedStatus(ONE))).toBeVisible();
  });

  await test.step('"Unblock" takes the label off at once, without a dialog', async () => {
    await members.row(ONE).getByRole('button', { name: `${W.unblock} ${ONE}` }).click();
    await expect(members.badge(ONE, W.badgeBlocked)).toHaveCount(0);
    await expect(members.dialog).toBeHidden();
  });
});

test.describe('--me-excluded', () => {
  test.use({ targetFlags: { extraArgs: ['--me-excluded'] } });
  test.skip(({ targetKind }) => targetKind === 'node', 'starting removed is a flag of the demo node; skipped before the target starts');

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
  test.skip(({ targetKind }) => targetKind === 'node', 'the waiting node is a flag of the demo node; skipped before the target starts');

  test('88-members-admit: a pending node, an admin lets it in, the row becomes a member', async ({ app, page, targetKind }) => {
    test.skip(targetKind === 'node', 'the waiting node is a flag of the demo node');
    const members = new MembersSection(page);
    await app.tabs.open('peers');

    await test.step('the request is listed apart with the label and the "Confirm…" button', async () => {
      await expect(members.pendingList).toContainText(W.pendingTitle);
      await expect(members.pendingRow(ANNA)).toContainText(W.badgePending);
      await expect(members.pendingRow(ANNA)).toContainText(W.labelLine);
      await expect(members.row(ANNA)).toHaveCount(0);
    });

    await test.step('"Confirm…" opens the dialog; Cancel changes nothing', async () => {
      await members.pendingRow(ANNA).getByRole('button', { name: W.admitAria(ANNA) }).click();
      await expect(members.dialogTitle(W.admitTitle(ANNA))).toBeVisible();
      await members.dialogButton(W.dialogCancel).click();
      await expect(members.dialog).toBeHidden();
      await expect(members.pendingRow(ANNA)).toBeVisible();
    });

    await test.step('confirming: the node is admitted at once, the row moves to the list', async () => {
      await members.pendingRow(ANNA).getByRole('button', { name: W.admitAria(ANNA) }).click();
      await members.confirmYes().click();
      await expect(members.pendingList).toHaveCount(0);
      await expect(members.row(ANNA)).toBeVisible();
      await expect(members.statusText(W.admittedStatus(ANNA))).toBeVisible();
    });
  });
});

test.describe('--me-pending', () => {
  test.use({ targetFlags: { extraArgs: ['--me-pending'] } });
  test.skip(({ targetKind }) => targetKind === 'node', 'starting as a waiting node is a flag of the demo node; skipped before the target starts');

  test('89-members-me-pending: a newcomer sees that an admin has to let it in', async ({ app, page, targetKind }) => {
    test.skip(targetKind === 'node', 'starting as a waiting node is a flag of the demo node');
    const members = new MembersSection(page);
    await app.tabs.open('peers');
    await expect(members.pendingCard).toBeVisible();
    await expect(members.secondAdminHint).toHaveCount(0);
  });
});
