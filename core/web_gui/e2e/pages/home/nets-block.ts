import type { Locator, Page } from '@playwright/test';

import { NET_NAME } from '../glossary';

/** Texts of the network choice, verbatim as the page shows them. */
export const NETS_TEXT = {
  /** Block heading (in Settings and in the step 2 expansion). */
  title: 'Which local networks can connect',
  /** Names by role on the router. */
  homeNet: 'Home network',
  providerNet: 'ISP network',
  /** A grey badge for an automatic refusal; a badge for the network through which the page is open. */
  badgeAutoClosed: 'closed automatically',
  badgeHere: 'you are here now',
  /** Under the list, while no list is set (the automatic choice is in effect). */
  autoNote: 'The networks are currently chosen automatically.',
  /** The step 2 summary in automatic mode (the "Edit" button next to it). */
  summaryAuto: 'The proxy is open for: Home network. The ISP network is closed automatically.',
  /** A warning under the checked ISP network. */
  warnProvider:
    "Then any device on the network the router is connected to will be able to use the proxy without a password: for example, on the main home router's network or your ISP's. Select only if that is also your network.",
  /** A note under the network through which the page is open. */
  hereNote:
    "This page is open through this network — you can't deselect it here. Open the page from another network or from this computer itself.",
  /** The intro paragraph of step 2 on the router and the former one on the laptop. */
  pacIntroRouter: `Enter this address once in the proxy settings of a phone or computer connected to the router. After that, the browser opens sites through ${NET_NAME} by itself whenever you choose an exit. No password needed.`,
  pacIntroLaptop: `Enter this address once in your computer's proxy settings. After that, the browser opens sites through ${NET_NAME} by itself whenever you choose an exit. No password needed.`,
} as const;

/** Background color of an element as numbers: from `rgb(r, g, b)` or `rgba(r, g, b, a)`; transparent — `a = 0`. */
export interface Rgba {
  r: number;
  g: number;
  b: number;
  a: number;
}

const RGB_PATTERN = /^rgba?\((\d+), (\d+), (\d+)(?:, ([\d.]+))?\)$/;

/**
 * Network choice for the proxy and the page: the step 2 summary of the setup on the router
 * with the "Edit" button that expands the network list, and the same list in Settings. The
 * "Setup" tab is here too: the summary lives only in step 2 of the setup.
 * The PAC address and the step's intro paragraph are in `HomeProxyCard` (`app.proxyCard.addressText`, `.hint`).
 */
export class NetsBlock {
  /** The "Setup" tab on Home: without a setup mark it opens step 2. */
  readonly wizardTab: Locator;
  // no anchor: the summary is a paragraph without a role or label, and its text is the subject of the check, so it cannot be found by it
  readonly summary: Locator;
  /** "Edit" in the summary; `aria-expanded` shows whether the network list is expanded. */
  readonly editButton: Locator;
  // no anchor: the block root has no role or label, `nets-block`; in Settings and in step 2 it is the same
  readonly block: Locator;
  /** The block heading is found by text, not by `id`: the block's absence is checked with it too. */
  readonly title: Locator;
  readonly saveButton: Locator;
  // no anchor: a small line without a role or label, `nets-auto-note`; its text is the subject of the check
  readonly autoNote: Locator;
  // no anchor: the note under the "you are here now" line, no role or label, `nets-here-note`
  readonly hereNote: Locator;
  // no anchor: the line "From the same networks as the proxy" in "Settings → Page", no role or label, `same-nets`
  readonly sameNets: Locator;
  /** The mode lists in Settings: "Who can use the proxy" (main) and "Who can access the settings page" (for a specialist). */
  readonly socksAccess: Locator;
  readonly webAccess: Locator;

  constructor(private readonly page: Page) {
    this.wizardTab = page.getByRole('tab', { name: 'Setup', exact: true });
    this.summary = page.locator('#nets-summary');
    this.editButton = page.getByRole('button', { name: 'Edit', exact: true });
    this.block = page.locator('#nets-block');
    this.title = page.getByText(NETS_TEXT.title);
    this.saveButton = this.block.getByRole('button', { name: 'Save', exact: true });
    this.autoNote = page.locator('#nets-auto-note');
    this.hereNote = page.locator('#nets-here-note');
    this.sameNets = page.locator('#same-nets');
    this.socksAccess = page.getByRole('combobox', { name: 'Who can use the proxy' });
    this.webAccess = page.getByRole('combobox', { name: 'Who can access the settings page' });
  }

  /** Opens the "Setup" tab of Home: for a node in a network without a setup mark this is step 2. */
  async openWizard(): Promise<void> {
    await this.wizardTab.click();
  }

  /**
   * A network row by its human-readable name ("Home network", "ISP network"): a label with a checkbox,
   * the name, a small explanation, and a badge on the right.
   */
  row(title: string): Locator {
    // no anchor: the row has no role or name of its own; a `<label>` with a checkbox inside
    return this.block.locator('label').filter({ hasText: title });
  }

  checkbox(title: string): Locator {
    return this.row(title).getByRole('checkbox');
  }

  /** The grey badge on the right of a network row: "closed automatically", "not selected", "you are here now". */
  badge(title: string, text: string): Locator {
    return this.row(title).getByText(text, { exact: true });
  }

  /** The yellow warning under the checked ISP network: `name` is the interface name, `eth0`. */
  warning(ifaceName: string): Locator {
    // no anchor: the warning is a paragraph without a role or label, `net-warn-<interface name>`
    return this.page.locator('#net-warn-' + ifaceName);
  }

  /** Background of an element — a measurement, not a selector (`getComputedStyle` on an already found element). */
  async background(locator: Locator): Promise<Rgba> {
    const css = await locator.evaluate((el) => getComputedStyle(el).backgroundColor);
    const m = RGB_PATTERN.exec(css);
    if (!m) throw new Error(`background color is not in the rgb() form: ${css}`);
    return { r: Number(m[1]), g: Number(m[2]), b: Number(m[3]), a: m[4] === undefined ? 1 : Number(m[4]) };
  }
}
