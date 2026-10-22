// Application fixture — the page and its sections.
import { mergeTests } from '@playwright/test';

import { AppShell } from '../pages/app-shell';
import { test as consoleTest } from './console';
import { test as targetTest } from './target';

export const test = mergeTests(targetTest, consoleTest).extend<{ app: AppShell }>({
  app: async (
    // eslint-disable-next-line @typescript-eslint/no-unused-vars -- listeners before goto
    { page, target, consoleLog },
    use,
  ) => {
    const app = new AppShell(page);
    await app.open(target.origin, target.token);
    await use(app);
  },
});
