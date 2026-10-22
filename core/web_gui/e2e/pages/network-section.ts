import type { Locator, Page } from '@playwright/test';

declare global {
  interface Window {
    __clip?: string | null;
  }
}

/** The "Diagnostics" tab (section): traffic light, events, summary, details for a specialist. */
export class DiagnosticsSection {
  readonly checks: Locator;
  readonly summaryButton: Locator;
  readonly summaryStatus: Locator;
  readonly specialist: Locator;
  readonly keyCard: Locator;
  readonly keyShowButton: Locator;
  readonly keyHideButton: Locator;
  /** The invite button — on Home, not on Diagnostics; needed by the invite tests. */
  readonly inviteButton: Locator;
  /** The invite line — on Home, see `inviteButton`. */
  readonly inviteValue: Locator;

  constructor(private readonly page: Page) {
    this.checks = page.getByRole('list', { name: 'Checks' });
    this.summaryButton = page.getByRole('button', { name: 'Copy summary for help' });
    this.summaryStatus = page.getByRole('status').filter({ hasText: /Summary|Clipboard/ });
    // `<summary>` has no role in Playwright: look it up by text (which is also the label of the collapsible block)
    this.specialist = page.getByText('Technical details', { exact: true });
    this.keyCard = page.getByRole('region', { name: 'Mesh Town network key' });
    this.keyShowButton = this.keyCard.getByRole('button', { name: 'Show', exact: true });
    this.keyHideButton = this.keyCard.getByRole('button', { name: 'Hide', exact: true });
    this.inviteButton = page.getByRole('button', { name: 'Invite a member' });
    this.inviteValue = page.getByLabel('Invite to send');
  }

  /** Expands the `<details>` "Technical details" if collapsed. */
  async openSpecialist(): Promise<void> {
    if (await this.keyCard.isVisible()) {
      return;
    }
    await this.specialist.click();
  }

  /** Replaces the clipboard write: the last line settles in `window.__clip` (`null` — nothing was written). */
  async trapClipboard(): Promise<void> {
    await this.page.evaluate(() => {
      window.__clip = null;
      navigator.clipboard.writeText = (s: string) => {
        window.__clip = s;
        return Promise.resolve();
      };
    });
  }

  async clipboard(): Promise<string | null> {
    return this.page.evaluate(() => window.__clip ?? null);
  }
}
