import { expect, test, type Locator, type Page } from '@playwright/test';

import { gotoReady } from '../ready';
import { TAB_NAMES } from '../tabs-bar';
import { HomeProxyCard } from './proxy-card';

/**
 * The Home section: panel, card boxes, first screen — bottom, presence, proxy hint, node mark,
 * address navigation. The computer name is `HomeNameCard`; the proxy hint (incl. the "Your browser" group)
 * and its locators are `HomeProxyCard`; the one copy here is not duplicated.
 */
export class HomeSection {
  readonly root: Locator;
  /** The "How sites open right now" panel (only with a network). */
  readonly statePanel: Locator;
  /** The main text of the status panel — where sites go right now. */
  readonly routeState: Locator;
  /** The "Three steps…" setup (without a network and after joining, until the steps are done). */
  readonly wizard: Locator;
  readonly tabList: Locator;
  readonly tabItems: Locator;

  private readonly proxy: HomeProxyCard;

  constructor(private readonly page: Page) {
    this.proxy = new HomeProxyCard(page);
    this.root = page.getByRole('tabpanel', { name: TAB_NAMES.home, exact: true, includeHidden: true });
    this.statePanel = this.root.getByRole('region', { name: 'How sites open right now' });
    // the panel's main text is the only place with an id: it has neither a role nor a label
    this.routeState = this.statePanel.locator('#route-state');
    this.wizard = this.root.getByRole('region', { name: /^Three steps/ });
    this.tabList = page.locator('nav#tabs');
    this.tabItems = this.tabList.getByRole('tab');
  }

  /** The invite field of the join card inside the given panel (the card heading changes depending on whether there is a network). */
  joinInput(scope: Locator): Locator {
    return scope.getByLabel('Invite you received');
  }

  /** Returns the bottom of the locator's box (`y + height`); no box means the element is not rendered. */
  async bottom(locator: Locator): Promise<number> {
    const box = await locator.boundingBox();
    if (!box) {
      throw new Error(`no box: ${locator}`);
    }
    return box.y + box.height;
  }

  /** Target check of the proxy card: demo — the browser card with the "Copy address" button (the PAC address), node — "Invite a member" on Home. */
  async expectTargetSpecific(targetKind: string, home: Locator): Promise<void> {
    if (targetKind === 'demo') {
      await test.step('proxy is on: the browser card is in place and has "Copy address"', async () => {
        await expect.soft(this.proxy.card).toHaveCount(1);
        await expect.soft(this.proxy.copyButton).toBeVisible();
      });

      return;
    }

    await test.step('node is in a network: "Invite a member" is visible on the first screen, and there is no join card on Home', async () => {
      await expect.soft(home.getByRole('button', { name: 'Invite a member', exact: true })).toBeVisible();
      await expect.soft(this.joinInput(home)).toHaveCount(0);
    });
  }

  /** Opens the address again (a full load) and waits for page readiness (was `HomeUx.gotoAndWait`). */
  async gotoAndWait(url: string): Promise<void> {
    await gotoReady(this.page, url);
  }

  async hash(): Promise<string> {
    return this.page.evaluate(() => window.location.hash);
  }

  async back(): Promise<void> {
    await this.page.goBack();
  }

  /** Window 1280x800, scroll to the very bottom; the top of the tab strip relative to the window. */
  async tabsTopAfterScroll(tabs: Locator): Promise<{ top: number; viewportHeight: number }> {
    await this.page.setViewportSize({ width: 1280, height: 800 });
    await this.page.evaluate(() => window.scrollTo(0, 1e6));
    const box = await tabs.boundingBox();
    if (!box) {
      throw new Error('no box for the tab strip');
    }
    return { top: box.y, viewportHeight: this.page.viewportSize()?.height ?? 0 };
  }
}
