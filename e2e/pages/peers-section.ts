import { expect, test, type Locator, type Page } from '@playwright/test';

import { GLOSSARY } from './glossary';

/** Labels that can sit between a member's name and the country (removal vote, block). */
const MEMBER_BADGES = 'Removal proposed|Blocked on this computer';

/** What a member row action shows: "yes" — a button or a mark, "no" — refuses, "—" — nothing. */
export type PeerExitAction = 'yes' | 'no' | '—';

/** The "Members" section: the list of people (`ul#peers-list`); rows are found by code (`p.id` from `/api/state`). */
export class PeersSection {
  readonly list: Locator;
  readonly items: Locator;

  constructor(page: Page) {
    // A list without a name that contains the own row "You: …" (other lists on the page have no such row).
    this.list = page.getByRole('list').filter({ has: page.getByRole('listitem').filter({ hasText: GLOSSARY.selfPrefix }) });
    this.items = this.list.getByRole('listitem');
  }

  /** Checkboxes and switches in a row: other members' rows must not have them. */
  pickControls(row: Locator): Locator {
    return row.getByRole('checkbox').or(row.getByRole('radio'));
  }

  /** The member row by code. */
  row(code: string): Locator {
    return this.items.filter({ hasText: code });
  }

  /** The exit button in a row. */
  exitButton(row: Locator): Locator {
    return row.getByRole('button', { name: GLOSSARY.exitViaButton });
  }

  /** The row action: "yes" — an exit choice button or mark; "no" — "Not available as an exit"; "—" — neither. */
  async expectAction(row: Locator, action: PeerExitAction, label: string): Promise<void> {
    const can = row.getByText(GLOSSARY.exitChosenOk).or(row.getByText(GLOSSARY.exitChosenDown)).or(this.exitButton(row));
    const refused = row.getByText(GLOSSARY.exitRefused);
    await expect.soft(can, label).toHaveCount(action === 'yes' ? 1 : 0);
    await expect.soft(refused, label).toHaveCount(action === 'no' ? 1 : 0);
  }

  /** Returns the country in rows by node name; if no rows exist for any name, a skip annotation is added. */
  async expectCountries(byName: Record<string, string>): Promise<void> {
    let checked = 0;
    for (const [name, country] of Object.entries(byName)) {
      const row = this.items.filter({ hasText: name });
      if ((await row.count()) > 0) {
        checked += 1;
        await expect.soft(row.first(), name).toContainText(new RegExp(`${name}(?:${MEMBER_BADGES})?\\s+${country}(?![A-Z])`));
      }
    }
    if (checked === 0) {
      test.info().annotations.push({ type: 'skip', description: 'not checked: this target has no peer-one/peer-two rows' });
    }
  }

  /** Checks the program version in demo rows: peer-one — 0.0.9 and "older" (the hint names both versions), peer-two (version=null) — no version, no mark, no "null"; if there are no rows, a skip annotation is added. */
  async expectVersions(ownVersion: string): Promise<void> {
    const one = this.items.filter({ hasText: 'peer-one' });
    const two = this.items.filter({ hasText: 'peer-two' });
    let checked = 0;
    if ((await one.count()) > 0) {
      checked += 1;
      await this.expectOlderRow(one.first(), ownVersion);
    }
    if ((await two.count()) > 0) {
      checked += 1;
      await this.expectNoVersionRow(two.first());
    }
    if (checked === 0) {
      test.info().annotations.push({ type: 'skip', description: 'not checked: this target has no peer-one/peer-two rows' });
    }
  }

  private async expectOlderRow(row: Locator, ownVersion: string): Promise<void> {
    const title = `This member has Mesh Town 0.0.9, you have ${ownVersion}. Ask them to update Mesh Town.`;
    await expect.soft(row.locator('.peer-version'), 'peer-one: version').toHaveText('0.0.9');
    await expect.soft(row.locator('.peer-older'), 'peer-one: mark').toHaveText('older version');
    await expect.soft(row.locator('.peer-older'), 'peer-one: mark hint').toHaveAttribute('title', title);
  }

  private async expectNoVersionRow(row: Locator): Promise<void> {
    await expect.soft(row.locator('.peer-version'), 'peer-two: no version').toHaveCount(0);
    await expect.soft(row.locator('.peer-older'), 'peer-two: no mark').toHaveCount(0);
    await expect.soft(row, 'peer-two: no "null" in the row').not.toContainText('null');
  }
}
