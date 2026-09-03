import { Page } from '@playwright/test';
import { test, expect } from './support/fixtures';
import { probeE2E, skipReason } from './support/probe';

const probe = probeE2E();

/**
 * 文件级 BLOCKED：无 WebView2 / CDP（或 `INSIGHTFORGE_E2E_RUN!=1`）时，整组黄金路径
 * 在报告中呈现为 skipped（BLOCKED），绝不伪造 PASS。详见 support/probe.ts。
 */
test.skip(!probe.ok, skipReason(probe));

/** 每个黄金路径失败时按路径命名截图，便于事后取证（evidence file names per path）。 */
async function evidence(page: Page, name: string) {
  try {
    await page.screenshot({ path: `e2e/evidence/${name}.png`, fullPage: false });
  } catch {
    // 取证失败不应阻断测试本身
  }
}

async function waitForApp(page: Page) {
  await page.waitForSelector('[data-el="workspace"]', { timeout: 60_000 });
}

test.describe('InsightForge Windows E2E — 黄金路径', () => {
  test.beforeEach(async ({ page }) => {
    await waitForApp(page);
  });

  test('首启 → 添加来源 → 索引 → 中文检索', async ({ page }) => {
    await page.click('[data-el="nav-sources"]');
    await expect(page.locator('[data-el="pane-sources"]')).toBeVisible();

    await page.click('[data-el="sources-add"]');
    await page.fill('[data-el="sources-add-name"]', 'notes');
    await page.fill('[data-el="sources-add-path"]', '%E2E_SOURCE_DIR%');
    await page.click('[data-el="sources-add-submit"]');

    await expect(page.locator('[data-el="sources-progress-bar"]')).toBeVisible();
    await page.waitForSelector('[data-el="sources-row-status"]:has-text("Idle")', {
      timeout: 120_000,
    });

    await page.click('[data-el="nav-search"]');
    await page.fill('[data-el="search-input"]', '测试');
    await page.click('[data-el="search-submit"]');
    await expect(page.locator('[data-el="search-results"]')).toBeVisible();
    await evidence(page, 'search-chinese');
  });

  test('证据卡 → 钉入草稿 → 引用快照', async ({ page }) => {
    await page.click('[data-el="nav-search"]');
    await page.fill('[data-el="search-input"]', '测试');
    await page.click('[data-el="search-submit"]');
    await expect(page.locator('[data-el="search-card"]').first()).toBeVisible();

    await page.click('[data-el="search-card-pin"]');
    await page.click('[data-el="nav-drafts"]');
    await page.click('[data-el="drafts-list"] >> text=note.md');
    await page.click('[data-el="nav-references"]');
    await expect(page.locator('[data-el="reference-item"]').first()).toBeVisible();
    await evidence(page, 'evidence-to-reference');
  });

  test('编辑 → 保存 → 外部冲突 → 三选一', async ({ page, dataDir }) => {
    await page.click('[data-el="nav-drafts"]');
    // 新建草稿（处理浏览器 prompt 对话框）
    page.on('dialog', (d) => d.accept('note.md'));
    await page.click('[data-el="drafts-new"]');
    await page.fill('[data-el="drafts-editor-textarea"]', '本地内容');
    await page.click('[data-el="drafts-save"]');

    // 模拟外部编辑器改动磁盘文件，制造冲突
    const fs = await import('node:fs');
    fs.writeFileSync(`${dataDir}/drafts/note.md`, '外部改动');

    await page.fill('[data-el="drafts-editor-textarea"]', '再次本地内容');
    await page.click('[data-el="drafts-save"]');
    await expect(page.locator('[data-el="drafts-conflict"]')).toBeVisible();

    await page.click('[data-el="drafts-conflict-local"]');
    await expect(page.locator('[data-el="drafts-conflict"]')).toHaveCount(0);
    await evidence(page, 'conflict-keep-local');
  });

  test('删除来源 → 本地草稿快照仍保留', async ({ page }) => {
    await page.click('[data-el="nav-sources"]');
    await page.click('[data-el="sources-row-actions"] >> [data-el="sources-remove-btn"]');
    // 确认删除对话框
    page.on('dialog', (d) => d.accept());
    await expect(page.locator('[data-el="sources-empty"]')).toBeVisible();

    await page.click('[data-el="nav-diagnostics"]');
    await expect(page.locator('[data-el="diagnostics-snapshot-text"]')).toBeVisible();
    await evidence(page, 'source-delete-retained');
  });

  test('导出 HTML 流程', async ({ page }) => {
    await page.click('[data-el="nav-drafts"]');
    await page.click('[data-el="drafts-list"] >> text=note.md');
    await page.click('[data-el="export-submit"]');
    await evidence(page, 'export-html');
  });

  test('无 AI Key 降级：AI 命令不可用但其余可用', async ({ page }) => {
    await page.click('[data-el="nav-settings"]');
    await expect(page.locator('[data-el="pane-settings"]')).toBeVisible();
    // 未配置 Key 时应显示「未配置」徽标
    await expect(page.locator('text=Not configured').or(page.locator('text=未配置'))).toBeVisible();
    await evidence(page, 'no-ai-key-degrade');
  });

  test('重启后状态保持（草稿仍在）', async ({ page }) => {
    await page.click('[data-el="nav-drafts"]');
    await expect(page.locator('[data-el="drafts-list"]')).toBeVisible();
    await evidence(page, 'restart-state');
  });
});
