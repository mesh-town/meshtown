import { test, expect } from '../fixtures/index';
import { DomainsSection } from '../pages/domains-section';

test('41-home-exit-toggle-and-domain-placeholder: toggle #field-exit-enabled — text on the left, toggle on the right; the domain add field placeholder gives an example address, not the jargon "domain"', async ({ app, page }) => {
  const name = app.nameCard;
  const domains = new DomainsSection(page);

  await test.step('#home-exit-server .toggle-inline: the toggle is not the first child, justify-content: space-between', async () => {
    await expect.soft(name.exitToggleRow).toHaveCount(1);
    expect.soft(await name.layout()).toEqual({ toggleIsFirstChild: false, justifyContent: 'space-between' });
  });

  await test.step('#domain-add-input: the placeholder is an example site address, not the jargon "domain"', async () => {
    await app.tabs.open('domains'); // the field is in the section, its role is visible only on the open tab
    await expect.soft(domains.addInput).toHaveAttribute('placeholder', 'site address, for example example.com');
  });
});
