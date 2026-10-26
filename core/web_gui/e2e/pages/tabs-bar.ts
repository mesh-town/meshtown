import { expect, type Locator, type Page } from '@playwright/test';

export const SECTIONS = ['home', 'peers', 'domains', 'network', 'log', 'settings'] as const;
export type Section = (typeof SECTIONS)[number];

const EXPECT_SHOWN_TIMEOUT_MS = 2000;

/** Section → visible tab name; the only place where names are listed. */
export const TAB_NAMES: Record<Section, string> = {
  home: 'Home',
  peers: 'Networks',
  domains: 'Site checks',
  network: 'Diagnostics',
  log: 'Log',
  settings: 'Settings',
};

/** Narrow screen (`max-width: 720px`): the rare sections hide under the "More ▾" button and "Site checks" is called "Sites". */
export const NARROW_MAX_WIDTH = 720;
export const NARROW_MORE: readonly Section[] = ['network', 'log'];
const NARROW_TAB_NAMES: Partial<Record<Section, string>> = { domains: 'Sites' };

/** Tab strip (`role=tablist`) and navigation between page sections. */
export class TabsBar {
  readonly root: Locator;
  /** The "More ▾" button of the narrow strip; after choosing a section from the menu it reads "<section> ▾". */
  readonly moreButton: Locator;
  /** The open list of the "More" menu. */
  readonly menu: Locator;
  readonly menuItems: Locator;

  constructor(private readonly page: Page) {
    this.root = page.locator('nav#tabs');
    this.moreButton = this.root.getByText(/▾$/).first();
    this.menu = page.getByRole('menu');
    this.menuItems = this.menu.getByRole('menuitem');
  }

  /** True while the window is narrow and the sections of the "More" menu are not tabs of the strip. */
  private underMore(section: Section): boolean {
    const width = this.page.viewportSize()?.width ?? Number.POSITIVE_INFINITY;
    return width <= NARROW_MAX_WIDTH && NARROW_MORE.includes(section);
  }

  private nameOf(section: Section): RegExp {
    const names = [TAB_NAMES[section], NARROW_TAB_NAMES[section]].filter(Boolean).join('|');
    return new RegExp(`^(${names})(\\s|$)`);
  }

  /** The tab by name; "Networks" can carry the counter of requests that wait for this node ("Networks 1 request needs your confirmation"). On a narrow screen a "More" section is an item of the open menu. */
  tab(section: Section): Locator {
    if (this.underMore(section)) {
      return this.page.locator(`#tab-${section}`);
    }
    return this.root.getByRole('tab', { name: this.nameOf(section) });
  }

  /** Returns the section panel by tab name (`aria-labelledby`); hidden panels are counted by the cases, hence `includeHidden`. */
  panel(section: Section): Locator {
    if (this.underMore(section)) {
      // A closed menu has no item to name the panel by `aria-labelledby`.
      return this.page.locator(`#sec-${section}`);
    }
    return this.page.getByRole('tabpanel', { name: this.nameOf(section), includeHidden: true });
  }

  async click(section: Section): Promise<void> {
    if (this.underMore(section) && !(await this.menu.isVisible())) {
      await this.moreButton.click();
    }
    await this.tab(section).click();
  }

  /** Hard wait (like `click_nav`): the tab is selected and its panel is visible. */
  async expectShown(section: Section): Promise<void> {
    if (this.underMore(section)) {
      // The menu item is not a tab: the section shows in the label of the "More" button.
      await expect(this.moreButton).toContainText(TAB_NAMES[section], { timeout: EXPECT_SHOWN_TIMEOUT_MS });
    } else {
      await expect(this.tab(section)).toHaveAttribute('aria-selected', 'true', {
        timeout: EXPECT_SHOWN_TIMEOUT_MS,
      });
    }
    await expect(this.panel(section)).toBeVisible({ timeout: EXPECT_SHOWN_TIMEOUT_MS });
  }

  /**
   * Opens a section. If the tab or panel is not in the DOM — a soft failure with a label and `false`
   * (like `click_nav`: `add_missing`). Otherwise a click and the same condition as `expectShown`,
   * but as a soft wait — the timeout is swallowed, the test goes on, and it returns `true`.
   */
  async open(section: Section): Promise<boolean> {
    const entry = this.underMore(section) ? this.moreButton : this.tab(section);
    if ((await entry.count()) === 0 || (await this.panel(section).count()) === 0) {
      expect.soft(false, `element not in the DOM: tab "${TAB_NAMES[section]}"`).toBeTruthy();
      return false;
    }
    await this.click(section);
    await this.waitShownSoft(section);
    return true;
  }

  private async waitShownSoft(section: Section): Promise<void> {
    try {
      await this.expectShown(section);
    } catch {
      // A preparatory wait with no label — the error is swallowed, the test goes on.
    }
  }

  /** The strip scrolls: `scrollWidth` and `clientWidth` of the strip (a value function — cannot be expressed with a locator). */
  async scrollMetrics(): Promise<{ scrollWidth: number; clientWidth: number }> {
    return this.root.evaluate((el) => ({ scrollWidth: el.scrollWidth, clientWidth: el.clientWidth }));
  }
}
