import { expect, type Locator, type Page } from '@playwright/test';

import { GLOSSARY } from './glossary';
import { lastState } from './ready';

/** The "Site checks" section: the header of the domain table. */
export class DomainsSection {
  /** Returns the header cells of the active (visible) tab: "Site", "For you", one column per member (without a name — "member <id>"), and an empty one for actions. */
  readonly headers: Locator;

  /** The regular "Check all" page button — the size reference. */
  readonly checkAllButton: Locator;
  /**
   * The "Recheck" button of the table's first row. Its accessible name is "Recheck <site>"
   * (`aria-label`, the visible content is "⟳"); there are several rows with such a button, and `first()`
   * is deliberate: it takes a sample.
   */
  readonly rowRecheckButton: Locator;
  /** The "Site address" field (placeholder "site address, for example example.com"); the role is visible only while the tab is open. */
  readonly addInput: Locator;
  /** The "Add" button of the site address form. */
  readonly addButton: Locator;
  /** On a narrow screen: one card per site (`div.domain-card`) instead of the table. */
  readonly cards: Locator;
  /** The first card's "Check again" button (its accessible name is the text or "Recheck <site>"). */
  readonly cardRecheckButton: Locator;
  /** The "Remove" buttons of all cards. */
  readonly cardRemoveButtons: Locator;
  /** The open tab panel: hint texts and the table. */
  private readonly panel: Locator;

  constructor(private readonly page: Page) {
    this.checkAllButton = page.getByRole('button', { name: /Check all|Checking…/ });
    this.rowRecheckButton = page.getByRole('table').getByRole('button', { name: /^Recheck / }).first();
    // Hidden panels are not seen by getByRole — only the th of the open section are taken.
    this.panel = page.getByRole('tabpanel');
    this.headers = this.panel.getByRole('columnheader');
    this.cards = page.locator('div.domain-card');
    this.cardRecheckButton = this.cards.getByRole('button', { name: /^(Check again|Recheck )/ }).first();
    this.cardRemoveButtons = this.cards.getByRole('button', { name: /^Remove/ });
    this.addInput = page.getByRole('textbox', { name: 'Site address' });
    this.addButton = this.panel.getByRole('button', { name: 'Add', exact: true });
  }

  /** Returns the width of the whole page (`documentElement.scrollWidth`). */
  async pageScrollWidth(): Promise<number> {
    return this.page.evaluate(() => document.documentElement.scrollWidth);
  }

  /** Returns the right edge of an element in the window: `x + width` (0 if there is no box). */
  async rightEdge(target: Locator): Promise<number> {
    const box = await target.boundingBox();
    return box ? box.x + box.width : 0;
  }

  /** Returns the width of an element by its box. */
  async width(target: Locator): Promise<number> {
    return (await target.boundingBox())?.width ?? 0;
  }

  /**
   * Expected headers, from the body of the last `/api/state` response received by the page (the listener is `trackState`
   * from `AppShell.open`; a separate request could diverge from it — the demo node is live).
   */
  expectedHeaders(): string[] {
    const peers = lastState(this.page).peers ?? [];
    // A member column header is the name (without a name — "member") and the id on the line below.
    const labels = peers.filter((p) => !p.self).map((p) => `${p.name?.trim() || 'member'} ${p.id}`);
    return [GLOSSARY.siteColumn, GLOSSARY.youColumn, ...labels, ''];
  }

  /** The table header matched the page snapshot: polling until the next response catches up with the rendering. */
  async expectHeadersMatchSnapshot(): Promise<void> {
    await expect
      .poll(async () => {
        const got = (await this.headers.allInnerTexts()).map((t) => t.replace(/\s+/g, ' ').trim());
        const want = this.expectedHeaders();
        return JSON.stringify(got) === JSON.stringify(want) ? 'matched' : `header=${JSON.stringify(got)} snapshot=${JSON.stringify(want)}`;
      })
      .toBe('matched');
  }

  /** Returns the height of the button's box. */
  async height(button: Locator): Promise<number> {
    const box = await button.boundingBox();
    if (!box) {
      throw new Error(`no box: ${button}`);
    }
    return box.height;
  }

  /** Returns the style of the button's box (`border-style`) — computed CSS cannot be expressed with a locator. */
  async borderStyle(button: Locator): Promise<string> {
    return button.evaluate((el) => getComputedStyle(el).borderStyle);
  }
}
