import { expect, type Locator, type Page } from '@playwright/test';

import { MEMBERS_WORDS as W } from './glossary';

/** The member list of the Members sub-tab of the "Networks" tab: rows with the "⋯" menu, the second-admin hint, the group of former members, the confirmation dialog. */
export class MembersSection {
  readonly list: Locator;
  readonly secondAdminHint: Locator;
  readonly dialog: Locator;
  readonly excludedList: Locator;
  readonly excludedCard: Locator;
  readonly leftCard: Locator;
  readonly pendingList: Locator;
  readonly status: Locator;

  constructor(private readonly page: Page) {
    // no anchor: the member list has no accessible name
    this.list = page.locator('#peers-list');
    this.secondAdminHint = page.locator('#second-admin-hint');
    this.dialog = page.getByRole('alertdialog');
    // no anchor: a closed <details> has no role with a name; the summary text is found inside it
    this.excludedList = page.locator('details').filter({ hasText: 'No longer in the network' });
    this.excludedCard = page.getByRole('status').filter({ has: page.getByRole('heading', { name: W.excludedTitle }) });
    this.leftCard = page.getByRole('status').filter({ has: page.getByRole('heading', { name: W.leftTitle }) });
    this.pendingList = page.locator('#members-pending-list');
    // no anchor: the text a waiting computer sees about itself
    this.status = page.getByRole('status').filter({ hasText: /\S/ });
  }

  /** The row of a member by name. */
  row(name: string): Locator {
    return this.list.getByRole('listitem').filter({ has: this.page.getByText(name, { exact: true }) });
  }

  /** The row of a node that waits for confirmations, by name. */
  pendingRow(name: string): Locator {
    return this.pendingList.getByRole('listitem').filter({ has: this.page.getByText(name, { exact: true }) });
  }

  /** The action button of the open confirmation dialog. */
  confirmYes(): Locator {
    return this.page.locator('#member-confirm-yes');
  }

  /** The "⋯" button of a row. */
  menuButton(name: string): Locator {
    return this.page.getByRole('button', { name: W.moreActions(name) });
  }

  menuItem(label: string): Locator {
    return this.page.getByRole('menuitem', { name: label });
  }

  async openMenu(name: string): Promise<void> {
    await this.menuButton(name).click();
    await expect(this.page.getByRole('menu')).toBeVisible();
  }

  /** Opens the "⋯" menu of a member and picks an item. */
  async pick(name: string, label: string): Promise<void> {
    await this.openMenu(name);
    await this.menuItem(label).click();
  }

  dialogTitle(title: string): Locator {
    return this.dialog.getByRole('heading', { name: title });
  }

  dialogButton(label: string): Locator {
    return this.dialog.getByRole('button', { name: label, exact: true });
  }

  /** The "Leave network…" button of the Settings tab (the own row of the member list has the same item in its menu). */
  settingsLeaveButton(): Locator {
    return this.page.getByRole('tabpanel', { name: /^Settings/ }).getByRole('button', { name: W.menuLeave, exact: true });
  }

  badge(name: string, text: string): Locator {
    return this.row(name).getByText(text, { exact: true });
  }

  /** The counter on the "Networks" tab. */
  tabChip(count = 1): Locator {
    return this.page.getByRole('tab', { name: /^Networks/ }).getByLabel(W.chipLabelOf(count));
  }

  /** The text of a status line under the list. */
  statusText(text: string): Locator {
    return this.page.getByRole('status').filter({ hasText: text });
  }
}
