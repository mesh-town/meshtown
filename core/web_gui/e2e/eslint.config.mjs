import { defineConfig } from 'eslint/config';
import tseslint from 'typescript-eslint';
import playwright from 'eslint-plugin-playwright';

export default defineConfig([
  {
    ignores: ['node_modules'],
  },
  {
    files: ['**/*.ts'],
    extends: [tseslint.configs.recommended, playwright.configs['flat/recommended']],
    rules: {
      'playwright/no-skipped-test': ['error', { allowConditional: true }],
    },
  },
]);
