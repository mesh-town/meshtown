import { expect, type Locator, type Page } from '@playwright/test';

import { NET_NAME } from './glossary';
import { TopPanel } from './top-panel';

const DIRTY_TIMEOUT_MS = 6000;

/** The "Settings" section (the open tab panel) and the identity strip `#identity-strip` above the page. */
export class SettingsSection {
  readonly root: Locator;
  readonly tab: Locator;
  readonly intro: Locator;
  // The field is found by role and name: textbox "Proxy port (SOCKS5 and HTTP)".
  readonly socksPort: Locator;
  // id: a locked toggle without a label or name, except for the id `field-web-enabled`.
  readonly webEnabled: Locator;
  readonly restartBadge: Locator;
  readonly staleEnabledText: Locator;
  // id: the form fields have no label and no name, the only anchor is the id `field-checks-provider_dns`.
  readonly providerDns: Locator;
  // id: same, `field-checks-doh_server`.
  readonly dohServer: Locator;
  readonly saveButton: Locator;
  // id: the sticky save bar is a container without a role or label, `settings-save-bar`.
  readonly saveBar: Locator;
  // id: a toggle without a label or name, `field-checks-enabled`.
  readonly checksEnabled: Locator;
  // id: a toggle without a label or name, `field-socks-strict`.
  readonly socksStrict: Locator;
  // id: a list without a label or name, `field-checks-interval_sec`.
  readonly checksInterval: Locator;
  readonly resetButton: Locator;
  /** The text of the save result "Saved"; its class `saved` is the witness of success (doSave). */
  readonly savedMessage: Locator;
  // id: the reason paragraph has no role or label, its text is set by the node config, so it cannot be found by text.
  readonly writableReason: Locator;
  readonly conflictHeading: Locator;
  readonly conflictReload: Locator;
  /** The "Restart Mesh Town" button in the save bar. */
  readonly restartButton: Locator;
  /** The message after clicking the button: "Mesh Town is restarting. The settings will take effect in a few seconds." */
  readonly restartMessage: Locator;
  // id: the node status line in the header is a paragraph without a role or label, `status-line`.
  readonly statusLine: Locator;

  constructor(private readonly page: Page) {
    this.root = page.getByRole('tabpanel');
    this.tab = page.getByRole('tab', { name: /Settings/ });
    this.intro = this.root.getByText(/Fields marked "after restart" take effect after Mesh Town restarts/);
    this.socksPort = page.getByRole('textbox', { name: 'Proxy port (SOCKS5 and HTTP)' });
    this.webEnabled = page.locator('#field-web-enabled');
    this.restartBadge = page
      .getByText('Proxy port (SOCKS5 and HTTP)')
      .getByText('after restart');
    // A read-only boolean value used to be shown as the word "on".
    this.staleEnabledText = this.root.getByText('on', { exact: true });
    this.providerDns = page.locator('#field-checks-provider_dns');
    this.dohServer = page.locator('#field-checks-doh_server');
    // exact: next to it in the conflict strip there is "Save my changes over them".
    this.saveButton = this.root.getByRole('button', { name: 'Save', exact: true });
    this.saveBar = page.locator('#settings-save-bar');
    this.socksStrict = page.locator('#field-socks-strict');
    this.checksEnabled = page.locator('#field-checks-enabled');
    this.checksInterval = page.locator('#field-checks-interval_sec');
    this.resetButton = this.root.getByRole('button', { name: 'Discard changes' });
    this.savedMessage = this.root.getByText(/^Saved/);
    this.writableReason = page.locator('#settings-writable-reason');
    this.conflictHeading = this.root.getByRole('heading', { name: 'The settings changed while you were editing' });
    this.conflictReload = this.root.getByRole('button', { name: /Reload/ });
    this.restartButton = this.root.getByRole('button', { name: 'Restart Mesh Town' });
    this.restartMessage = this.root.getByText(new RegExp(`^${NET_NAME} is restarting`));
    this.statusLine = new TopPanel(page).statusLine;
  }

  /** Enters a port value and waits (the timeout is swallowed) for the "not saved" mark on the tab. */
  async typeSocksPort(value: string): Promise<void> {
    await this.socksPort.fill(value);
    try {
      await expect(this.tab).toHaveClass(/has-dirty/, { timeout: DIRTY_TIMEOUT_MS });
    } catch {
      // A preparatory wait — the mark check itself follows as a hard/soft expect.
    }
  }

  /** Expands the collapsed "Advanced" block: without it, its fields are invisible and cannot be typed into. */
  async openExpert(): Promise<void> {
    if (!(await this.providerDns.isVisible())) {
      await this.page.getByText('Advanced', { exact: true }).click();
    }
  }

  /** Types a value into the `checks.provider_dns` field and clicks "Save". */
  async saveProviderDns(value: string): Promise<void> {
    await this.openExpert();
    await this.providerDns.fill(value);
    await this.saveButton.click();
  }

  /** A field with no restart and no launch flag on the target: "How often to check sites" — saves "every 6 hours". */
  async saveChecksInterval(): Promise<void> {
    await this.checksInterval.selectOption({ label: 'every 6 hours' });
    await this.saveButton.click();
  }

  /** A pause longer than the 4-second timer that used to dismiss "Saved…". */
  async waitPastSavedTimer(): Promise<void> {
    await new Promise((resolve) => setTimeout(resolve, 5000));
  }

  /**
   * After clicking "Restart Mesh Town": the demo holds `starting` for 3 s and shows "Mesh Town
   * is starting…" in `#status-line`; the real node comes up faster than the poll — not checked there.
   */
  async expectStartingStatus(targetKind: string): Promise<void> {
    const expected: Record<string, RegExp | undefined> = { demo: new RegExp(`${NET_NAME} is starting…`) };
    const pattern = expected[targetKind];
    if (pattern) await expect(this.statusLine).toHaveText(pattern);
  }

  /** Returns the text of the tab's `::after` pseudo-element (the unsaved mark). */
  async tabAfterContent(): Promise<string> {
    return this.tab.evaluate((el) => getComputedStyle(el, '::after').content);
  }

  /** Colors of the grey badge, as given by a probe with the same `--grey-bg`/`--grey` tokens. */
  async greyTokens(): Promise<{ bg: string; fg: string }> {
    return this.root.evaluate(() => {
      const probe = document.createElement('span');
      probe.style.cssText = 'background:var(--grey-bg);color:var(--grey);position:absolute;visibility:hidden;';
      document.body.appendChild(probe);
      const style = getComputedStyle(probe);
      const tokens = { bg: style.backgroundColor, fg: style.color };
      probe.remove();
      return tokens;
    });
  }
}
