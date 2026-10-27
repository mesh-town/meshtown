import type { Locator, Page } from '@playwright/test';

/**
 * The "Connect the browser to Mesh Town" card on Home: the intro line, the autoconfiguration (PAC) address
 * with the "Copy address" button, the system switch, and the manual setup, collapsed by default.
 */
export class HomeProxyCard {
  readonly card: Locator;
  /** The card's intro line (with the proxy off — "The browser proxy is off…"). */
  readonly hint: Locator;
  /** The autoconfiguration address: `http://127.0.0.1:<port>/proxy.pac`. */
  readonly addressText: Locator;
  /** "Copy address" in the PAC address line (the manual block has its own button with the same name). */
  readonly copyButton: Locator;
  /** "Not working? Set up manually ▸" — expands the manual block (`aria-expanded`). */
  readonly manualToggle: Locator;
  /** The manual block: browser, steps, "Copy address" and "Copy port"; in the DOM only when expanded. */
  readonly manualSetup: Locator;
  readonly copyPortButton: Locator;
  /** The "Your system" group and its four buttons (macOS, Windows, Linux, Phone). */
  readonly osGroup: Locator;
  readonly osButtons: Locator;
  /** The pressed system button (`aria-pressed="true"`) — exactly one. */
  readonly osPressed: Locator;

  /** The scope question's two buttons: "In the whole system" and "Only in the browser". */
  readonly scopeSystem: Locator;
  readonly scopeBrowser: Locator;

  constructor(page: Page) {
    this.card = page.getByRole('region', { name: 'Connect the browser to Mesh Town', exact: true });
    // anchor — id: the hint paragraph has no role or label; its bottom is measured
    this.hint = page.locator('#proxy-hint');
    // anchor — id: the address, the copy buttons and the manual block have neither roles nor unique labels
    this.addressText = page.locator('#pac-url-text');
    this.copyButton = page.locator('#pac-copy-btn');
    this.manualToggle = page.locator('#manual-toggle');
    this.manualSetup = page.locator('#proxy-setup');
    this.copyPortButton = page.locator('#socks-copy-port-btn');
    this.scopeSystem = page.locator('#scope-system');
    this.scopeBrowser = page.locator('#scope-browser');
    this.osGroup = page.locator('#os-seg');
    this.osButtons = this.osGroup.getByRole('button');
    this.osPressed = this.osGroup.getByRole('button', { pressed: true });
  }

  /** Answers "Where should the proxy work?" so that the system steps or the Firefox steps appear. */
  async pickScope(scope: 'system' | 'browser'): Promise<void> {
    await (scope === 'system' ? this.scopeSystem : this.scopeBrowser).click();
  }
}
