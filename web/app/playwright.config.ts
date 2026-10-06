import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: 'tests',
  timeout: 240_000,
  expect: { timeout: 10_000 },
  globalSetup: './tests/global-setup.ts',
  use: { baseURL: 'http://localhost:4173', acceptDownloads: true },
  webServer: {
    command: 'npm run build && npm run preview',
    url: 'http://localhost:4173',
    timeout: 300_000,
    reuseExistingServer: true,
  },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
});
