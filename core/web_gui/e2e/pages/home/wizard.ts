import { expect, type Locator, type Page } from '@playwright/test';

import { NET_NAME } from '../glossary';

/** Texts of the step-by-step setup that specs compare verbatim. */
export const WIZARD_TEXT = {
  title: `Set up ${NET_NAME}`,
  scopeQuestion: `Where should ${NET_NAME} open sites?`,
  scopeSystem: 'All apps on this computer (recommended)',
  scopeBrowser: 'Only in one browser',
  osQuestion: 'Which system are you setting up?',
  browserQuestion: 'Which browser do you use?',
} as const;

export type WizardStep = 'join' | 'scope' | 'os' | 'browser' | 'guide' | 'exit' | 'connect';
export type WizardScope = 'system' | 'browser';
export type WizardOs = 'mac' | 'windows' | 'linux' | 'android' | 'ios';
export type WizardBrowser = 'chrome' | 'edge' | 'safari' | 'firefox' | 'other';
/** The id of a choice tile: `scope-<scope>`, `os-<os>`, `browser-<browser>`. */
export type WizardTile = `scope-${WizardScope}` | `os-${WizardOs}` | `browser-${WizardBrowser}`;

const SETUP_MARK_KEY = 'meshtown.homeSetup';

/**
 * The step-by-step setup on Home ("Setup" tab): one screen at a time, with a progress line, a question,
 * choice tiles and Back / Next / Skip / Finish. The Overview tab next to it is here too, to leave the setup.
 */
export class HomeWizard {
  readonly region: Locator;
  /** The "Setup" and "Overview" tabs of Home. */
  readonly setupTab: Locator;
  readonly overviewTab: Locator;
  /** The screen container; its `data-step` names the current step. */
  readonly screen: Locator;
  /** The heading of the current screen (it takes the focus after a step change). */
  readonly question: Locator;
  /** "Step N of M". */
  readonly progress: Locator;
  /** "✓ You joined the network …" — on the first screen after joining. */
  readonly joined: Locator;
  readonly backButton: Locator;
  readonly nextButton: Locator;
  readonly skipButton: Locator;
  readonly finishButton: Locator;
  /** The "More options" summary at the bottom of the instruction. */
  readonly moreOptionsSummary: Locator;
  /** Terms with a hint (PAC, SOCKS5) and the open hint. */
  readonly terms: Locator;
  readonly tooltip: Locator;

  constructor(private readonly page: Page) {
    this.region = page.getByRole('region', { name: WIZARD_TEXT.title, exact: true });
    this.setupTab = page.getByRole('tab', { name: 'Setup', exact: true });
    this.overviewTab = page.getByRole('tab', { name: 'Overview', exact: true });
    // anchor — id: the screen container and the heading have no role or label of their own
    this.screen = this.region.locator('#wizard-screen');
    this.question = this.region.locator('#wizard-q');
    this.progress = this.region.locator('#wizard-progress');
    this.joined = this.region.locator('#wizard-joined');
    this.backButton = this.region.getByRole('button', { name: 'Back', exact: true });
    this.nextButton = this.region.getByRole('button', { name: 'Next', exact: true });
    this.skipButton = this.region.getByRole('button', { name: 'Skip this step', exact: true });
    this.finishButton = this.region.getByRole('button', { name: 'Finish', exact: true });
    // no anchor: the summary of a details element has no role in the accessibility tree
    this.moreOptionsSummary = this.region.locator('details#more-options > summary');
    // no anchor: a term is a focusable span without a role; the hint has the tooltip role and is shown only while the term is hovered or focused
    this.terms = this.region.locator('.term');
    this.tooltip = this.region.getByRole('tooltip');
  }

  /** Opens the "Setup" tab. */
  async open(): Promise<void> {
    await this.setupTab.click();
  }

  /** Opens the "Overview" tab. */
  async openOverview(): Promise<void> {
    await this.overviewTab.click();
  }

  /** A choice tile of the current screen. */
  tile(id: WizardTile): Locator {
    // no anchor: a tile is a button whose label is a long sentence (scope) or repeats across screens; the id is stable
    return this.region.locator(`#${id}`);
  }

  async next(): Promise<void> {
    await this.nextButton.click();
  }

  async back(): Promise<void> {
    await this.backButton.click();
  }

  async skip(): Promise<void> {
    await this.skipButton.click();
  }

  /** "Finish" on the exit screen: closes the setup and opens Overview. */
  async finish(): Promise<void> {
    await this.finishButton.click();
  }

  async expectStep(step: WizardStep): Promise<void> {
    await expect(this.screen).toHaveAttribute('data-step', step);
  }

  /**
   * From the first question to the instruction. A tile leads on at once. With scope "browser" the system question
   * is skipped, so `os` is not used there; on a phone system there is no browser question (`browser` is not used).
   */
  async toGuide(scope: WizardScope, os: WizardOs, browser: WizardBrowser | null = null): Promise<void> {
    await this.expectStep('scope');
    await this.tile(`scope-${scope}`).click();
    const phone = scope === 'system' && (os === 'android' || os === 'ios');
    if (scope === 'system') {
      await this.expectStep('os');
      await this.tile(`os-${os}`).click();
    }
    if (!phone && browser) {
      await this.expectStep('browser');
      await this.tile(`browser-${browser}`).click();
    }
    await this.expectStep('guide');
  }

  /** The setup mark kept by the page (`null` — none). */
  async setupMark(): Promise<string | null> {
    return this.page.evaluate((key) => window.localStorage.getItem(key), SETUP_MARK_KEY);
  }
}
