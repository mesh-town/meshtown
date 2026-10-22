import { type Locator, type Page } from '@playwright/test';

/** Banners about the settings file state on the Settings tab: "can't be read" and "some settings could not be read". */
export class SettingsFileBanner {
  /** Heading of the red banner about a damaged file. */
  readonly brokenTitle: Locator;
  /** Heading of the grey banner about field errors (its only anchor is the hint label). */
  readonly fieldErrorsHint: Locator;

  /**
   * Banner container: the same error text is duplicated in the identity strip and the hidden hint
   * of the entry, so the search is narrowed to it.
   */
  private readonly root: Locator;

  constructor(page: Page) {
    this.root = page.getByRole('region', { name: 'Settings file' });
    this.brokenTitle = this.root.getByRole('heading', { name: "The settings file can't be read" });
    this.fieldErrorsHint = this.root.getByText('Some of the settings in the file');
  }

  /** Hint of the red banner: the error message starts with `prefix` (the banner heading also contains "can't be read", so the anchor is at the start). */
  brokenMessage(prefix: string): Locator {
    return this.root.getByText(new RegExp(`^${prefix}`));
  }

  /** A line of the field error list of the form "<field title> (<path>): <message>". */
  fieldError(text: string): Locator {
    return this.root.getByText(text);
  }
}
