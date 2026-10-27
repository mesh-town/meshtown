import type { Locator, Page } from '@playwright/test';

export interface ToggleLayout {
  toggleIsFirstChild: boolean;
  justifyContent: string;
}

/** The "This computer's name" and "Allow other members to open sites through this computer" cards on Home. */
export class HomeNameCard {
  readonly nameField: Locator;
  readonly nameStatus: Locator;
  readonly exitToggle: Locator;
  /** The toggle row: it has no role in the markup, so this is the checkbox itself, and `layout()` measures the row. */
  readonly exitToggleRow: Locator;

  constructor(page: Page) {
    this.nameField = page.getByRole('textbox', { name: "This computer's name" });
    // anchor — id: the card has several `role="status"` paragraphs; the status of exactly this field is needed
    this.nameStatus = page.locator('#field-node-name-status');
    this.exitToggle = page.getByRole('checkbox', { name: /^Allow other members to open sites through/ });
    this.exitToggleRow = this.exitToggle;
  }

  /** Fills in the name and confirms with Enter — autosave, as in `case_21`. */
  async rename(name: string): Promise<void> {
    await this.nameField.fill(name);
    await this.nameField.press('Enter');
  }

  /** Clears the name field and leaves it (blur) — the node substitutes a random name. */
  async clearAndBlur(): Promise<void> {
    await this.nameField.fill('');
    await this.nameField.blur();
  }

  /** The toggle is not the first child of the row, and `justify-content` (computed style — cannot be expressed with a locator). */
  async layout(): Promise<ToggleLayout> {
    return this.exitToggle.evaluate((el) => {
      const row = el.closest('label');
      return {
        toggleIsFirstChild: row?.firstElementChild?.classList.contains('toggle') ?? false,
        justifyContent: row ? getComputedStyle(row).justifyContent : '',
      };
    });
  }
}
