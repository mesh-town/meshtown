import type { Locator, Page } from '@playwright/test';
import { test, expect } from '../fixtures/index';
import type { MeshApi } from '../fixtures/api';
import { patchMainState } from '../helpers/overview-route';
import { MembersSection } from '../pages/members-section';

// Managed networks on the page. The demo node serves the same five members as in members.spec.ts; the flags below decide the kind and the roles:
//   --managed         a Managed network, you are an admin and its creator, peer-one is the second admin, peer-two is an ordinary member;
//   --managed-member  a Managed network, you are an ordinary member, peer-one and peer-two are the admins.
// A real node cannot be put into a role on demand, so every case is for the demo only.

const ADMIN_PEER = 'peer-one';
const PLAIN_PEER = 'peer-two';
const SECOND_ADMIN = 'peer-two';

const WORDS = {
  kindPeer: 'Peer network',
  kindManaged: 'Managed network',
  finalHint: "You can't change this after the network is created.",
  adminBadge: 'Admin',
  adminsLine: 'Managed network · Admins:',
  menuMakeAdmin: 'Make admin',
  menuMakeMember: 'Make regular member',
  menuRemove: 'Remove from network…',
  menuRemoveAdminsOnly: 'Remove from network (admins only)',
  menuBlock: 'Block on this computer…',
  menuBan: 'Ban from network…',
  menuRename: 'Rename',
  menuLeave: 'Leave network…',
  menuGiveUp: 'Give up admin rights',
  menuGiveUpOnly: "Give up admin rights (you're the only admin)",
  menuDelete: 'Delete network for everyone…',
  titleCreator: 'The creator of the network stays an admin',
  titleOnlyAdmin: 'Make another member an admin first',
  titleAdminsOnly: 'Only admins can do this',
  inviteNote: 'Only admins can invite people to',
  dialogCancel: 'Cancel',
  deleteField: 'Type the network name to confirm',
  deleteAction: 'Delete network',
  makeAdminAction: 'Make admin',
  makeAdminTitle: (name: string, net: string): string => `Make ${name} an admin of "${net}"?`,
  makeAdminStatus: (name: string): string => `${name} is now an admin of`,
  deleteTitle: (net: string): string => `Delete "${net}" for everyone?`,
  managedWarning: (net: string): string =>
    `If this computer's Mesh Town data is lost and nobody else is an admin, "${net}" can't be managed any more. Make a second admin soon.`,
};

test.describe('Managed network, picked on the create form (no network yet)', () => {
  test.skip(({ targetKind }) => targetKind === 'node', 'the kind picker is checked on the demo node, which starts without a network here');
  test.use({ targetFlags: { extraArgs: ['--no-network'] } });

  test('104-managed-kind-picker: Peer is the default; Managed shows the warning with the typed name; "Create network" sends the kind', async ({
    app,
    page,
  }) => {
    const picker = new KindPicker(page);
    await app.netCard.startCreate.click();
    await app.netCard.createLabelInput.fill('Office');

    await test.step('Peer is chosen, the choice is final, no warning yet', async () => {
      await expect(picker.peer).toBeChecked();
      await expect(picker.managed).not.toBeChecked();
      await expect(page.getByText(WORDS.finalHint)).toBeVisible();
      await expect(picker.warning).toHaveCount(0);
    });

    await test.step('choosing Managed shows the warning that names the typed network', async () => {
      await picker.choose(WORDS.kindManaged);
      await expect(picker.managed).toBeChecked();
      await expect(picker.warning).toHaveText(WORDS.managedWarning('Office'));
    });

    await test.step('going back to Peer takes the warning away', async () => {
      await picker.choose(WORDS.kindPeer);
      await expect(picker.warning).toHaveCount(0);
    });

    await test.step('"Create network" with Managed chosen sends the name and the kind', async () => {
      await picker.choose(WORDS.kindManaged);
      const created = page.waitForRequest((r) => r.method() === 'POST' && new URL(r.url()).pathname === '/api/network/create');
      await app.netCard.createButton.click();
      expect((await created).postDataJSON()).toEqual({ label: 'Office', kind: 'Managed' });
    });
  });
});

test.describe('--managed (you are an admin and the creator)', () => {
  test.skip(({ targetKind }) => targetKind === 'node', 'the roles are a flag of the demo node; skipped before the target starts');
  test.use({ targetFlags: { extraArgs: ['--managed'] } });

  test.describe('the Members tab', () => {
    test('105-managed-admins: the admins line and the "Admin" badge; the menu of someone else\'s row has the admin actions', async ({ app, page }) => {
      const members = new MembersSection(page);
      await app.tabs.open('peers');

      await test.step('the line names the admins; you are one of them', async () => {
        const line = page.locator('#admins-line');
        await expect(line).toContainText(WORDS.adminsLine);
        await expect(line).toContainText('you');
        await expect(line).toContainText(ADMIN_PEER);
        await expect(line).not.toContainText(PLAIN_PEER);
      });

      await test.step('the badge is on the admins, not on the ordinary member', async () => {
        await expect(members.badge(ADMIN_PEER, WORDS.adminBadge)).toBeVisible();
        await expect(ownRow(page).getByText(WORDS.adminBadge, { exact: true })).toBeVisible();
        await expect(members.badge(PLAIN_PEER, WORDS.adminBadge)).toHaveCount(0);
      });

      await test.step('an ordinary member: make admin, remove, block; no ban vote', async () => {
        await members.openMenu(PLAIN_PEER);
        await expect(item(page, WORDS.menuMakeAdmin)).toBeEnabled();
        await expect(item(page, WORDS.menuRemove)).toBeEnabled();
        await expect(item(page, WORDS.menuBlock)).toBeEnabled();
        await expect(item(page, WORDS.menuBan)).toHaveCount(0);
        await page.keyboard.press('Escape');
      });

      await test.step('another admin: the role can be taken back', async () => {
        await members.openMenu(ADMIN_PEER);
        await expect(item(page, WORDS.menuMakeMember)).toBeEnabled();
        await expect(item(page, WORDS.menuMakeAdmin)).toHaveCount(0);
      });
    });

    test('107-managed-make-admin: "Make admin" asks first; confirming gives the row the "Admin" badge', async ({ app, page, api }) => {
      const members = new MembersSection(page);
      const net = await netLabel(api);
      await app.tabs.open('peers');

      await test.step('the dialog names the member and the network; Cancel changes nothing', async () => {
        await members.pick(SECOND_ADMIN, WORDS.menuMakeAdmin);
        await expect(members.dialogTitle(WORDS.makeAdminTitle(SECOND_ADMIN, net))).toBeVisible();
        await members.dialogButton(WORDS.dialogCancel).click();
        await expect(members.dialog).toBeHidden();
        await expect(members.badge(SECOND_ADMIN, WORDS.adminBadge)).toHaveCount(0);
      });

      await test.step('confirming: the badge, the status and the admins line follow', async () => {
        await members.pick(SECOND_ADMIN, WORDS.menuMakeAdmin);
        await members.dialogButton(WORDS.makeAdminAction).click();
        await expect(members.badge(SECOND_ADMIN, WORDS.adminBadge)).toBeVisible();
        await expect(members.statusText(WORDS.makeAdminStatus(SECOND_ADMIN))).toBeVisible();
        await expect(page.locator('#admins-line')).toContainText(SECOND_ADMIN);
      });

      await test.step('the menu of that row now offers to take the role back', async () => {
        await members.openMenu(SECOND_ADMIN);
        await expect(item(page, WORDS.menuMakeMember)).toBeEnabled();
      });
    });

    test('108-managed-last-admin: the creator keeps the role; with another admin around "Give up" works, as the only admin it is grey with the reason', async ({
      app,
      page,
      api,
      target,
    }) => {
      const members = new MembersSection(page);
      await app.tabs.open('peers');

      await test.step('given two admins: "Give up admin rights" is available', async () => {
        await openOwnMenu(page);
        await expect(item(page, WORDS.menuGiveUp)).toBeEnabled();
        await page.keyboard.press('Escape');
      });

      await test.step('the creator of the network: "Make regular member" is grey and says why', async () => {
        // In --managed you are the creator, so the creator's row is never on screen: the state is rewritten so that peer-one is the creator.
        const creator = await idOf(api, ADMIN_PEER);
        await patchMainState<NetState>(page, (state) => ({ ...state, network: { ...state.network, root: creator } }));
        await app.open(target.origin, target.token);
        await app.tabs.open('peers');
        await members.openMenu(ADMIN_PEER);
        const demote = item(page, WORDS.menuMakeMember);
        await expect(demote).toBeDisabled();
        await expect(demote).toHaveAttribute('title', WORDS.titleCreator);
        await page.unroute('**/api/overview');
        await app.open(target.origin, target.token);
        await app.tabs.open('peers');
      });

      await test.step('when the other admin loses the role (no dialog): you are the only admin', async () => {
        await members.pick(ADMIN_PEER, WORDS.menuMakeMember);
        await expect(members.badge(ADMIN_PEER, WORDS.adminBadge)).toHaveCount(0);
        await expect(members.dialog).toBeHidden();
      });

      await test.step('then "Give up admin rights" is grey and says to choose a successor', async () => {
        await openOwnMenu(page);
        const giveUp = item(page, WORDS.menuGiveUpOnly);
        await expect(giveUp).toBeDisabled();
        await expect(giveUp).toHaveAttribute('title', WORDS.titleOnlyAdmin);
      });
    });

    test('109-managed-delete-network: "Delete network for everyone…" needs the network name typed; afterwards the network is empty', async ({
      app,
      page,
      api,
    }) => {
      const members = new MembersSection(page);
      const net = await netLabel(api);
      await app.tabs.open('peers');

      await test.step('the own menu has the item; the dialog names the network', async () => {
        await openOwnMenu(page);
        await item(page, WORDS.menuDelete).click();
        await expect(members.dialogTitle(WORDS.deleteTitle(net))).toBeVisible();
      });

      await test.step('the red button waits for the exact name', async () => {
        const field = members.dialog.getByLabel(WORDS.deleteField);
        const action = members.dialogButton(WORDS.deleteAction);
        await expect(action).toBeDisabled();
        await field.fill(net.slice(0, -1));
        await expect(action).toBeDisabled();
        await field.fill(net);
        await expect(action).toBeEnabled();
      });

      await test.step('confirming closes the dialog; the other members are gone', async () => {
        await members.dialogButton(WORDS.deleteAction).click();
        await expect(members.dialog).toBeHidden();
        await expect(members.row(ADMIN_PEER)).toHaveCount(0);
        await expect(members.row(PLAIN_PEER)).toHaveCount(0);
      });
    });
  });
});

test.describe('--managed-member (you are an ordinary member)', () => {
  test.skip(({ targetKind }) => targetKind === 'node', 'the roles are a flag of the demo node; skipped before the target starts');
  test.use({ targetFlags: { extraArgs: ['--managed-member'] } });

  test('106-managed-member: a note instead of the invite button; the menus have the admin actions grey', async ({ app, page }) => {
    const members = new MembersSection(page);
    await app.tabs.open('peers');
    const panel = app.tabs.panel('peers');

    await test.step('Members: the note tells who to ask; there is no invite button', async () => {
      await expect(panel.locator('#peers-invite-note')).toContainText(WORDS.inviteNote);
      await expect(panel.locator('#peers-invite-btn')).toHaveCount(0);
      await expect(page.locator('#admins-line')).toContainText(WORDS.adminsLine);
      await expect(ownRow(page).getByText(WORDS.adminBadge, { exact: true })).toHaveCount(0);
    });

    await test.step("an admin's row: block is there, remove is grey with the reason", async () => {
      await members.openMenu(ADMIN_PEER);
      await expect(item(page, WORDS.menuBlock)).toBeEnabled();
      const remove = item(page, WORDS.menuRemoveAdminsOnly);
      await expect(remove).toBeDisabled();
      await expect(remove).toHaveAttribute('title', WORDS.titleAdminsOnly);
      await expect(item(page, WORDS.menuMakeAdmin)).toHaveCount(0);
      await page.keyboard.press('Escape');
    });

    await test.step('the own menu: rename and leave, no admin actions', async () => {
      await openOwnMenu(page);
      await expect(item(page, WORDS.menuRename)).toBeEnabled();
      await expect(item(page, WORDS.menuLeave)).toBeEnabled();
      await expect(item(page, WORDS.menuGiveUp)).toHaveCount(0);
      await expect(item(page, WORDS.menuDelete)).toHaveCount(0);
    });

    await test.step('Home: the invite card has the same note and no button', async () => {
      await app.tabs.open('home');
      const home = app.tabs.panel('home');
      await expect(home.locator('#peers-invite-note')).toContainText(WORDS.inviteNote);
      await expect(home.locator('#netkey-invite-btn')).toHaveCount(0);
    });
  });
});

interface NetState {
  network?: Record<string, unknown>;
}

interface MembersDoc {
  members?: { id_full: string; name?: string }[];
}

interface StateDoc {
  network?: { label?: string | null } | null;
}

/** The kind choice of the create form: two radio cards and the warning under the Managed one. */
class KindPicker {
  readonly peer: Locator;
  readonly managed: Locator;
  readonly warning: Locator;

  constructor(private readonly page: Page) {
    this.peer = page.locator('#kind-peer');
    this.managed = page.locator('#kind-managed');
    this.warning = page.locator('#kind-managed-warn');
  }

  /** Clicks the card by its title; the radio itself is a 1px invisible input. */
  async choose(title: string): Promise<void> {
    await this.page.locator('label.kcard').filter({ hasText: title }).click();
  }
}

/** The row of this computer in the member list. */
function ownRow(page: Page): Locator {
  return page.locator('#peers-list .peer-self');
}

async function openOwnMenu(page: Page): Promise<void> {
  await ownRow(page).getByRole('button', { name: /^More actions for/ }).click();
  await expect(page.getByRole('menu')).toBeVisible();
}

/** An item of the open "⋯" menu by its exact text. */
function item(page: Page, label: string): Locator {
  return page.getByRole('menuitem', { name: label, exact: true });
}

/** The name of the network the demo node serves, as the node reports it. */
async function netLabel(api: MeshApi): Promise<string> {
  const state = await api.getJson<StateDoc>('/api/state');
  const label = state.body?.network?.label;
  expect(label, 'the demo network has a name').toBeTruthy();
  return label as string;
}

/** The full id of a member by name, from the member list of the node. */
async function idOf(api: MeshApi, name: string): Promise<string> {
  const list = await api.getJson<MembersDoc>('/api/members');
  const row = list.body?.members?.find((m) => m.name === name);
  expect(row, `${name} is in the member list`).toBeTruthy();
  return (row as { id_full: string }).id_full;
}
