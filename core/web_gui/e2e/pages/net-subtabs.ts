import { expect, type Locator, type Page } from '@playwright/test';

export type NetSubId = 'members' | 'services' | 'device';

/** Sections of one network's tab on the Networks page: Members, Shared services (ports), Device settings. */
export class NetSubTabs {
  readonly root: Locator;
  readonly panel: Locator;

  constructor(private readonly page: Page) {
    this.root = page.locator('#net-subtabs');
    this.panel = page.locator('#net-subpanel');
  }

  tab(sub: NetSubId): Locator {
    return this.page.locator(`#net-sub-${sub}`);
  }

  /** Clicks the sub-tab and waits until it is selected. */
  async open(sub: NetSubId): Promise<void> {
    await this.tab(sub).click();
    await expect(this.tab(sub)).toHaveAttribute('aria-selected', 'true', { timeout: 2000 });
    await expect(this.panel).toBeVisible({ timeout: 2000 });
  }

  /** Right edge of the sub-tab strip (a value function — cannot be expressed with a locator). */
  async scrollMetrics(): Promise<{ scrollWidth: number; clientWidth: number }> {
    return this.root.evaluate((el) => ({ scrollWidth: el.scrollWidth, clientWidth: el.clientWidth }));
  }
}
