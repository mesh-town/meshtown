import type { Locator, Page } from '@playwright/test';

/** The "Ports" tab (port publishing). */
export class PortsSection {
  readonly addPort: Locator;
  readonly addName: Locator;
  readonly addButton: Locator;
  readonly audienceAll: Locator;
  readonly audienceNodes: Locator;
  readonly ownBody: Locator;
  readonly netRows: Locator;

  constructor(private readonly page: Page) {
    // Field names are their placeholders ("port", "name (optional)"); exact — the "Ports shared with you" rows have a "port number on this computer" field.
    this.addPort = page.getByRole('spinbutton', { name: 'port', exact: true });
    this.addName = page.getByRole('textbox', { name: 'name (optional)' });
    this.addButton = page.getByRole('button', { name: 'Publish', exact: true });
    this.audienceAll = page.getByRole('radio', { name: 'All members' });
    this.audienceNodes = page.getByRole('radio', { name: 'Selected members' });
    // Tables are told apart by their accessible name (`aria-label` on `<table>`).
    this.ownBody = page.getByRole('table', { name: 'My published ports' });
    this.netRows = page
      .getByRole('table', { name: 'Ports shared with you' })
      .getByRole('row')
      .filter({ has: page.getByRole('button', { name: /^(Open on this computer|Close)$/ }) });
  }

  async publish(port: string, name: string): Promise<void> {
    await this.addPort.fill(port);
    await this.addName.fill(name);
    await this.addButton.click();
  }

  /** The node's checkbox in the "Selected members" list by node name (the label starts with the name). */
  audienceNode(name: string): Locator {
    return this.page.getByRole('checkbox', { name: new RegExp(`^${name}`) });
  }

  localInput(row: Locator): Locator {
    return row.getByRole('spinbutton', { name: 'port number on this computer' });
  }

  openButton(row: Locator): Locator {
    return row.getByRole('button', { name: 'Open on this computer', exact: true });
  }

  closeButton(row: Locator): Locator {
    return row.getByRole('button', { name: 'Close', exact: true });
  }
}
