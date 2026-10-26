import type { Locator, Page } from '@playwright/test';

import type { AppShell } from './app-shell';
import { TabsBar } from './tabs-bar';

/** Card region: the heading "Join with an invite" without a network, no card of this kind with one (a link to the "+" tab takes its place). */
const CARD_NAME = /^(Join with an invite|Join or create another network)/;

/**
 * The join card: the invite field, the "Join"/"Switch" button, the error line. The section draws it on
 * Home without a network and, with a network, "Settings → Network" has only a link (`#net-join-slot`) to the "+" tab.
 */
export class JoinCard {
  readonly keyInput: Locator;
  readonly joinButton: Locator;
  readonly keyError: Locator;
  /** The card inside the Home panel. */
  readonly inHomeSlot: Locator;
  /** The card inside the Settings panel. */
  readonly inNetSlot: Locator;
  /** The invite field inside the card on Settings. */
  readonly netSlotKeyInput: Locator;
  /** The link to the "+" tab in the Settings slot (`#net-join-slot`); it replaces the join card once a network exists. */
  readonly addNetLink: Locator;

  constructor(page: Page) {
    this.keyInput = page.getByLabel('Invite you received');
    // The label depends on the network: "Join" without a network, "Switch" with one (section).
    this.joinButton = page.getByRole('button', { name: /^(Join|Switch)$/ });
    this.keyError = page.getByRole('alert', { name: 'Invite error', includeHidden: true });
    const tabs = new TabsBar(page);
    this.inHomeSlot = tabs.panel('home').getByRole('region', { name: CARD_NAME, includeHidden: true });
    this.inNetSlot = tabs.panel('settings').getByRole('region', { name: CARD_NAME, includeHidden: true });
    this.netSlotKeyInput = this.inNetSlot.getByLabel('Invite you received');
    this.addNetLink = page.locator('#net-join-slot a[href="#peers/add"]');
  }

  /**
   * The field is not visible (the card is on Settings) — switch to the Settings tab,
   * otherwise a click on a field inside a hidden section hangs.
   */
  async reveal(app: AppShell): Promise<void> {
    if (await this.keyInput.isVisible()) {
      return;
    }
    await app.tabs.open('settings');
  }
}
