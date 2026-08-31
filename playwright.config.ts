import { defineConfig, devices } from '@playwright/test';

const INSTALLER_PATH = process.env.INSIGHTFORGE_INSTALLER_PATH || './target/installer/InsightForge-Setup.exe';
const WEBVIEW2_PORT = parseInt(process.env.INSIGHTFORGE_WEBVIEW2_PORT || '9222');

export default defineConfig({
  testDir: './e2e',
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: 1,
  reporter: [['html', { outputFolder: 'e2e-report' }], ['json', { outputFile: 'e2e-results.json' }]],
  use: {
    baseURL: `http://127.0.0.1:${WEBVIEW2_PORT}`,
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: `"${INSTALLER_PATH}" --silent`,
    url: `http://127.0.0.1:${WEBVIEW2_PORT}`,
    reuseExistingServer: !process.env.CI,
    timeout: 120 * 1000,
  },
});
