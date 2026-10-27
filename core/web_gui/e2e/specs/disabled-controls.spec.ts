import { test, expect } from '../fixtures/index';

const NARROW = { width: 390, height: 844 };

// Elements are added to the live page, so the styles are the ones the real stylesheet gives.
async function addMarkup(page: import('@playwright/test').Page, html: string): Promise<void> {
  await page.evaluate((markup) => {
    const host = document.createElement('div');
    host.id = 'probe-host';
    host.innerHTML = markup;
    document.body.append(host);
  }, html);
}

const style = (page: import('@playwright/test').Page, selector: string, props: string[]) =>
  page.locator(selector).evaluate((el, names) => {
    const css = getComputedStyle(el);
    return names.map((n) => css.getPropertyValue(n));
  }, props);

test('114-disabled-controls: a disabled .btn and .btn-x look dimmed with a plain cursor, active ones are unchanged', async ({ app, page }) => {
  void app;
  await addMarkup(
    page,
    '<button id="p-btn-off" class="btn" disabled>A</button><button id="p-btn-on" class="btn">B</button>' +
      '<button id="p-x-off" class="btn-x" disabled>C</button><button id="p-x-on" class="btn-x">D</button>',
  );
  for (const id of ['p-btn-off', 'p-x-off']) {
    expect.soft(await style(page, `#${id}`, ['opacity', 'cursor']), id).toEqual(['0.5', 'default']);
  }
  for (const id of ['p-btn-on', 'p-x-on']) {
    expect.soft((await style(page, `#${id}`, ['opacity']))[0], id).toBe('1');
  }
});

test('115-btn-text-underline: at 390 px the text button of a domain card is underlined, an icon .btn-x is not', async ({ app, page }) => {
  await app.resize(NARROW);
  await addMarkup(page, '<button id="p-text" class="btn-x btn-text">Text</button><button id="p-icon" class="btn-x">x</button>');
  const [text] = await style(page, '#p-text', ['text-decoration-line']);
  const [icon] = await style(page, '#p-icon', ['text-decoration-line']);
  expect.soft(text, 'text button').toContain('underline');
  expect.soft(icon, 'icon button').not.toContain('underline');
});

test('116-disabled-link-controls: a disabled .linkbtn, .nets-link, .step-pick and exit radio look dimmed, active ones are unchanged', async ({ app, page }) => {
  void app;
  await addMarkup(
    page,
    '<button id="p-lb-off" class="linkbtn" disabled>A</button><button id="p-lb-on" class="linkbtn">B</button>' +
      '<button id="p-nl-off" class="nets-link" disabled>C</button><button id="p-nl-on" class="nets-link">D</button>' +
      '<button id="p-sp-off" class="step-pick" disabled>E</button><button id="p-sp-on" class="step-pick">F</button>' +
      '<input id="p-ec-off" type="radio" class="exit-choice" disabled><input id="p-ec-on" type="radio" class="exit-choice">',
  );
  for (const id of ['p-lb-off', 'p-nl-off', 'p-sp-off', 'p-ec-off']) {
    expect.soft(await style(page, `#${id}`, ['opacity', 'cursor']), id).toEqual(['0.5', 'default']);
  }
  for (const id of ['p-lb-on', 'p-nl-on', 'p-sp-on', 'p-ec-on']) {
    expect.soft((await style(page, `#${id}`, ['opacity']))[0], id).toBe('1');
  }
});
