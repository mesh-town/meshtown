import { expect, type Locator, type Page } from '@playwright/test';

const SETTLE_TIMEOUT_MS = 3000;

/** The "Log" section: the log line window (`role=log`), lines are `listitem`s with a time. */
export class LogSection {
  readonly window: Locator;
  readonly lines: Locator;

  constructor(page: Page) {
    this.window = page.getByRole('log', { name: 'Mesh Town log' });
    this.lines = this.window.getByRole('listitem').filter({ has: page.getByText(/^\+\s*\d+\.\d+s$/) });
  }

  /** Soft wait (the timeout is swallowed) for at least one line — the timeout is swallowed, the test goes on. */
  async settle(): Promise<void> {
    try {
      await expect(this.lines.first()).toBeAttached({ timeout: SETTLE_TIMEOUT_MS });
    } catch {
      // A preparatory wait with no label — the error is swallowed, the test goes on.
    }
  }
}
