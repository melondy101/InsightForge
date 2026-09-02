import { defineConfig, devices } from '@playwright/test';

// InsightForge E2E: 仅能在 Windows 10/11 + WebView2 + 签名安装包环境执行
// 当前 WSL 环境自动跳过

const APP_URL = process.env.INSIGHTFORGE_APP_URL || 'http://localhost:1420'; // dev fallback，实际使用 WebView2 CDP

export default defineConfig({
  testDir: './e2e',
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: 1,
  reporter: [
    ['html', { outputFolder: 'e2e-report', open: 'never' }],
    ['json', { outputFile: 'e2e-results.json' }],
  ],
  use: {
    baseURL: APP_URL,
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
    // 允许在 Windows 环境通过 CDP 连接 WebView2
    ...(process.env.INSIGHTFORGE_CDP_ENDPOINT ? { cdpConnection: { endpoint: process.env.INSIGHTFORGE_CDP_ENDPOINT } } : {}),
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  // 注意：Tauri 安装包不是 HTTP server，webServer 配置已移除
  // 真实自动化需通过 WebView2 CDP 连接
  // 见 e2e/README.md
});
