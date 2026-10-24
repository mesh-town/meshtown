import { expect, type Locator, type Page } from '@playwright/test';

/** The places that carry the one "Join with an invite" form; each gives its fields their own ids. */
export type InviteFormPlace = 'home' | 'add' | 'rejoin';

const IDS: Record<InviteFormPlace, { input: string; button: string; error: string; preview: string; note: string; dup: string; open: string; status?: string; refused?: string }> = {
  home: { input: 'join-key-input', button: 'join-btn', error: 'join-key-error', preview: 'join-kind-preview', note: 'join-place-note', dup: 'join-dup', open: 'join-open', status: 'join-status', refused: 'join-refused' },
  add: { input: 'add-net-input', button: 'add-net-btn', error: 'add-net-error', preview: 'add-net-kind-preview', note: 'add-net-place-note', dup: 'add-net-dup', open: 'add-net-open' },
  rejoin: { input: 'rejoin-input', button: 'rejoin-btn', error: 'rejoin-error', preview: 'rejoin-kind-preview', note: 'rejoin-place-note', dup: 'rejoin-dup', open: 'rejoin-open', status: 'rejoin-status', refused: 'rejoin-refused' },
};

/**
 * One instance of the invite form: on Home / in the wizard (`home`), in "Add another network" (`add`, Settings and the "+" tab),
 * under the card of a network tab this computer is out of (`rejoin`). Several forms share the document, so every locator
 * takes the visible one.
 */
export class InviteForm {
  readonly input: Locator;
  readonly button: Locator;
  readonly error: Locator;
  /** What the pasted string is: the kind of the network. */
  readonly kindPreview: Locator;
  /** What the node will do with the string here: "already in", "brings this computer back". */
  readonly placeNote: Locator;
  /** The "already in" block and its "Open" button. */
  readonly alreadyHere: Locator;
  readonly openButton: Locator;
  /** The line under the button that tells what was sent (home and rejoin only). */
  readonly status: Locator;
  /** The refusal of the core under the button (home and rejoin only): a bold lead and the text of the core. */
  readonly refused: Locator;
  /** The one-line note about the limit of networks, above the field, and the short error under it. */
  readonly limitLine: Locator;

  constructor(page: Page, place: InviteFormPlace) {
    const ids = IDS[place];
    // no anchor: the same form sits in several places of the document, only the ids tell them apart
    const visible = (id: string): Locator => page.locator(`#${id}:visible`);
    this.input = visible(ids.input);
    this.button = visible(ids.button);
    this.error = visible(ids.error);
    this.kindPreview = visible(ids.preview);
    this.placeNote = visible(ids.note);
    this.alreadyHere = visible(ids.dup);
    this.openButton = visible(ids.open);
    this.status = visible(ids.status ?? ids.dup);
    this.refused = visible(ids.refused ?? ids.error);
    this.limitLine = visible('add-net-limit');
  }

  /** Pastes an invite into the field. */
  async paste(invite: string): Promise<void> {
    await this.input.fill(invite);
  }

  /** Pastes an invite and waits until the page has told what it leads to (the preview line appears). */
  async pasteAndInspect(invite: string): Promise<void> {
    await this.paste(invite);
    await expect(this.kindPreview).toBeVisible();
  }

  async submit(): Promise<void> {
    await this.button.click();
  }
}

/** The tab row of the Members section: one tab per visible network and the "+" tab. */
export class NetTabs {
  readonly row: Locator;
  readonly slotTabs: Locator;
  readonly addTab: Locator;

  constructor(private readonly page: Page) {
    // no anchor: the row has no accessible name; the tabs carry the slot in a data attribute
    this.row = page.locator('#peers-net-tabs');
    this.slotTabs = this.row.locator('a.ntab[data-slot]');
    this.addTab = this.row.locator('a.ntab.add');
  }

  tab(slot: string): Locator {
    return this.row.locator(`[data-slot="${slot}"]`);
  }

  /** The slot ids of the visible network tabs, in the order of the row. */
  async slots(): Promise<string[]> {
    return this.slotTabs.evaluateAll((tabs) => tabs.map((t) => t.getAttribute('data-slot') ?? ''));
  }

  async hash(): Promise<string> {
    return this.page.evaluate(() => location.hash);
  }
}

/** The card of a computer that is out of a network, on Home (`excluded-*`) or on the Members tab (`members-removed-*`). */
export class OutOfNetCard {
  readonly card: Locator;
  readonly joinButton: Locator;

  constructor(page: Page, where: 'home' | 'members') {
    const prefix = where === 'home' ? 'excluded' : 'members-removed';
    // no anchor: the card is a status region with a heading that differs by variant; the prefix is the one stable name
    this.card = page.locator(`#${prefix}-card:visible`);
    this.joinButton = page.locator(`#${prefix}-join:visible`);
  }
}

/** The primary buttons on screen: a page keeps one main action. */
export function visiblePrimary(page: Page): Locator {
  return page.locator('.btn-primary:visible');
}
