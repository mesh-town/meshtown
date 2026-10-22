// Single entry point for specs — the shared `test`.
import { mergeTests } from '@playwright/test';

import { test as apiTest } from './api';
import { test as appTest } from './app';

export const test = mergeTests(apiTest, appTest);
export { expect } from '@playwright/test';
export type { ConfigDoc } from './api';
export type { TargetFlags, TargetKind } from './target';
