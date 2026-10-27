import { expect, type Locator, type Page, type ViewportSize } from '@playwright/test';
import { TabsBar, type Section } from './tabs-bar';
import { LogSection } from './log-section';
import { SettingsSection } from './settings-section';
import { SettingsFileBanner } from './settings-file-banner';
import { TopPanel } from './top-panel';
import { JoinCard } from './join-card';
import { HomeSection as HomePanel } from './home/section';
import { HomeNetCard } from './home/net-card';
import { HomeExitCard } from './home/exit-card';
import { HomeNameCard } from './home/name-card';
import { HomeProxyCard } from './home/proxy-card';
import { HomeWizard } from './home/wizard';
import { gotoReady } from './ready';

const RESIZE_TIMEOUT_MS = 3000;

/** The whole application: opening the page, tabs, the Home and Log sections. */
export class AppShell {
  readonly tabs: TabsBar;
  readonly log: LogSection;
  readonly settings: SettingsSection;
  readonly fileBanner: SettingsFileBanner;
  readonly topPanel: TopPanel;
  // Home objects, following the `pages/home/` map.
  readonly homePanel: HomePanel;
  readonly joinCard: JoinCard;
  readonly netCard: HomeNetCard;
  readonly exitCard: HomeExitCard;
  readonly nameCard: HomeNameCard;
  readonly proxyCard: HomeProxyCard;
  readonly wizard: HomeWizard;

  constructor(private readonly page: Page) {
    this.tabs = new TabsBar(page);
    this.log = new LogSection(page);
    this.settings = new SettingsSection(page);
    this.fileBanner = new SettingsFileBanner(page);
    this.topPanel = new TopPanel(page);
    this.homePanel = new HomePanel(page);
    this.joinCard = new JoinCard(page);
    this.netCard = new HomeNetCard(page);
    this.exitCard = new HomeExitCard(page);
    this.nameCard = new HomeNameCard(page);
    this.proxyCard = new HomeProxyCard(page);
    this.wizard = new HomeWizard(page);
  }

  /** Opens the page at the target's address and waits until it is ready (a timeout is a failure). */
  async open(origin: string, token: string): Promise<void> {
    await gotoReady(this.page, `${origin}/?token=${token}`);
  }

  /** Changes the window size (a timeout is a failure) — there is no way back to the original size. */
  async resize(size: ViewportSize): Promise<void> {
    await this.page.setViewportSize(size);
    await expect
      .poll(() => this.page.evaluate(() => window.innerWidth), { timeout: RESIZE_TIMEOUT_MS })
      .toBe(size.width);
  }

  /** For the Log tab — a soft wait for a line (the timeout is swallowed); other sections are ready right after being shown. */
  async settle(section: Section): Promise<void> {
    if (section === 'log') {
      await this.log.settle();
      return;
    }
  }

  /** Returns the section width on a narrow screen; if the tab is not in the DOM, fails softly inside and returns 0. */
  async sectionWidth(section: Section): Promise<number> {
    const shown = await this.tabs.open(section);
    if (!shown) {
      return 0;
    }
    await this.settle(section);
    return this.scrollWidth();
  }

  /** Returns the right edge of the section panel after opening it; if the tab is not in the DOM, fails softly inside and returns 0. */
  async panelRight(section: Section): Promise<number> {
    if (!(await this.tabs.open(section))) {
      return 0;
    }
    await this.settle(section);
    return this.right(this.tabs.panel(section));
  }

  async scrollWidth(): Promise<number> {
    return this.page.evaluate(() => document.documentElement.scrollWidth);
  }

  /** Returns the top of the locator's box (`boundingBox().y`) — no box means the element is not rendered. */
  async top(locator: Locator): Promise<number> {
    const box = await locator.boundingBox();
    if (!box) {
      throw new Error(`no box: ${locator}`);
    }
    return box.y;
  }

  /** Returns the right edge of the locator's box (`x + width`) — no box means the element is not rendered. */
  async right(locator: Locator): Promise<number> {
    const box = await locator.boundingBox();
    if (!box) {
      throw new Error(`no box: ${locator}`);
    }
    return box.x + box.width;
  }
}
