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

/** A screen of the wizard: the questions page, or one of the single steps. */
export type WizardScreen = 'choices' | 'join' | 'exit' | 'connect';
/** A row of the questions page. */
export type WizardRow = 'scope' | 'os' | 'browser' | 'guide';
export type WizardScope = 'system' | 'browser';
export type WizardOs = 'mac' | 'windows' | 'linux' | 'android' | 'ios';
export type WizardBrowser = 'chrome' | 'edge' | 'safari' | 'firefox' | 'other';
/** The id of a choice tile: `scope-<scope>`, `os-<os>`, `browser-<browser>`. */
export type WizardTile = `scope-${WizardScope}` | `os-${WizardOs}` | `browser-${WizardBrowser}`;

const SETUP_MARK_KEY = 'meshtown.homeSetup';

/**
 * The step-by-step setup on Home ("Setup" tab): one page with the questions one under another (the next row opens
 * when the one above is answered), then the exit screen; a progress line, choice tiles and Back / Next / Skip / Finish. The Overview tab next to it is here too, to leave the setup.
 */
export class HomeWizard {
  readonly region: Locator;
  /** The "Setup" and "Overview" tabs of Home. */
  readonly setupTab: Locator;
  readonly overviewTab: Locator;
  /** The screen container; its `data-step` names the current screen. */
  readonly screen: Locator;
  /** The heading of the exit or connect screen (it takes the focus after a screen change). */
  readonly question: Locator;
  /** The rows of the questions page, top to bottom; the last one is the open step. */
  readonly rows: Locator;
  /** The strip of steps beside (or above) the screen. */
  readonly strip: Locator;
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
    this.rows = this.region.locator('#wizard-screen .wizard-row');
    this.strip = this.region.locator('#wizard-strip');
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

  async expectScreen(screen: WizardScreen): Promise<void> {
    await expect(this.screen).toHaveAttribute('data-step', screen);
  }

  /** The heading of a row of the questions page. */
  rowHeading(row: WizardRow): Locator {
    // anchor — id: the heading has no label of its own besides its (long) question
    return this.region.locator(`#wizard-q-${row}`);
  }

  /** The questions page is open and its last (open) row is `row`. */
  async expectCursor(row: WizardRow): Promise<void> {
    await this.expectScreen('choices');
    await expect(this.rows.last()).toHaveAttribute('data-step', row);
  }

  /**
   * From the first question to the instruction. The answer to the scope opens every row below it (the system and the
   * browser are detected); a tile of `os` and `browser` is then pressed when given. Not used with scope "browser": `os`;
   * on a phone system there is no browser row.
   */
  async toGuide(scope: WizardScope, os: WizardOs, browser: WizardBrowser | null = null): Promise<void> {
    await this.expectCursor('scope');
    await this.tile(`scope-${scope}`).click();
    await this.expectCursor('guide');
    if (scope === 'system') await this.tile(`os-${os}`).click();
    if (browser && !(scope === 'system' && (os === 'android' || os === 'ios'))) await this.tile(`browser-${browser}`).click();
    await this.expectCursor('guide');
  }

  /** The setup mark kept by the page (`null` — none). */
  async setupMark(): Promise<string | null> {
    return this.page.evaluate((key) => window.localStorage.getItem(key), SETUP_MARK_KEY);
  }
}
