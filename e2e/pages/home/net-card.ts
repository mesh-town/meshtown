import type { Locator, Page } from '@playwright/test';

/**
 * The "network" card on Home: the choice panel without a network (two buttons "Join with an invite" and
 * "Create a new network"), network creation, and inviting a member.
 */
export class HomeNetCard {
  readonly startTitle: Locator;
  readonly startJoin: Locator;
  readonly startCreate: Locator;
  readonly createLabelInput: Locator;
  readonly createButton: Locator;
  readonly inviteButton: Locator;
  readonly inviteStatus: Locator;

  constructor(page: Page) {
    // the choice panel without a network: a heading and two buttons (`#home-start-choice`)
    this.startTitle = page.getByRole('heading', { name: "You haven't joined a Mesh Town network yet" });
    this.startJoin = page.getByRole('button', { name: 'Join with an invite', exact: true });
    this.startCreate = page.getByRole('button', { name: 'Create a new network', exact: true });
    // the name field and the "Create network" button are from `page`: they are in the form under the choice button
    this.createLabelInput = page.getByLabel(/Network name \(members will see it/);
    this.createButton = page.getByRole('button', { name: 'Create network', exact: true });
    this.inviteButton = page.getByRole('button', { name: 'Invite a member', exact: true });
    // anchor — id: a `role="status"` paragraph, but the card has several of them; it must be found even when hidden
    this.inviteStatus = page.locator('#invite-status');
  }

  /** Opens the create form, enters a name and clicks "Create network". */
  async createNetwork(title: string): Promise<void> {
    await this.startCreate.click();
    await this.createLabelInput.fill(title);
    await this.createButton.click();
  }
}
