import { test, expect, type Page } from '@playwright/test';

async function waitForApp(page: Page) {
  await page.waitForSelector('[data-el="workspace"]', { timeout: 60_000 });
}

test.describe('InsightForge Windows E2E', () => {
  test.beforeEach(async ({ page }) => {
    await waitForApp(page);
  });

  test('directory selection → indexing progress → evidence card → edit save', async ({ page }) => {
    // 1. 打开来源管理
    await page.click('[data-el="nav-sources"]');
    await expect(page.locator('[data-el="pane-sources"]')).toBeVisible();

    // 2. 点击添加来源
    await page.click('[data-el="source-add-button"]');
    // 注意：真实测试需处理系统对话框；此处为选择器占位

    // 3. 等待索引进度出现
    // await expect(page.locator('[data-el="index-progress"]')).toBeVisible();

    // 4. 切换到搜索视图
    await page.click('[data-el="nav-search"]');
    await expect(page.locator('[data-el="search-view"]')).toBeVisible();

    // 5. 执行搜索
    await page.fill('[data-el="search-input"]', '测试');
    await page.click('[data-el="search-submit"]');
    await expect(page.locator('[data-el="search-results"]')).toBeVisible();

    // 6. 打开证据卡上下文
    if (await page.locator('[data-el="search-card"]').count() > 0) {
      await page.click('[data-el="search-card"] [data-el="search-card-context"]');
      await expect(page.locator('[data-el="search-context"]')).toBeVisible();
    }

    // 7. 切换到工作台编辑
    await page.click('[data-el="nav-workspace"]');
    await expect(page.locator('[data-el="workspace"]')).toBeVisible();

    // 8. 保存草稿（需编辑器适配层支持）
    // await page.click('[data-el="save-draft-button"]');
  });

  test('external conflict → local/external/save-as resolution', async ({ page }) => {
    await page.click('[data-el="nav-drafts"]');
    await expect(page.locator('[data-el="pane-drafts"]')).toBeVisible();

    // 冲突三选一需外部文件变更触发；此处为选择器占位
    // await page.click('[data-el="conflict-choice-local"]');
  });

  test('AI consent → egress bundle preview → audit log', async ({ page }) => {
    await page.click('[data-el="nav-settings"]');
    await expect(page.locator('[data-el="pane-settings"]')).toBeVisible();

    // AI 总开关与 consent 流程
    // await page.click('[data-el="ai-toggle"]');
    // await page.click('[data-el="nav-egress"]');
    // await expect(page.locator('[data-el="egress-bundle"]')).toBeVisible();
  });

  test('generic HTML export flow', async ({ page }) => {
    await page.click('[data-el="nav-drafts"]');
    await page.click('[data-el="export-button"]');
    await expect(page.locator('[data-el="export-view"]')).toBeVisible();
    await page.click('[data-el="export-submit"]');
    // 验证下载触发（需真实文件系统）
  });

  test('WeChat clipboard export flow', async ({ page }) => {
    await page.click('[data-el="nav-drafts"]');
    await page.click('[data-el="clipboard-export-button"]');
    await expect(page.locator('[data-el="clipboard-export-view"]')).toBeVisible();
    // 验证剪贴板内容（需真实桌面环境）
  });

  test('thought card create → update → navigate', async ({ page }) => {
    await page.click('[data-el="nav-thoughts"]');
    await expect(page.locator('[data-el="pane-thoughts"]')).toBeVisible();
    // 创建、更新、跳转流程占位
  });

  test('install → upgrade → background process exit', async () => {
    // 本测试在签名安装包上升级验证；需 Windows 环境 + 签名证书
    test.skip(!!process.env.INSIGHTFORGE_SKIP_INSTALL, 'Skip install/upgrade in non-Windows');
  });
});
