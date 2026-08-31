# Handoff · Ticket 15 — Windows 全链路 E2E

- Issue：`.scratch/desktop-rewrite/issues/15-windows-e2e.md`
- 基线 SHA：`b2603c18`（integration/desktop-rewrite，含 03/07/14 修复）
- 分支：`ticket/15-windows-e2e`
- Worktree：`D:/Develop/wt/InsightForge/15-windows-e2e`
- 角色/会话：`Builder`，`orchestrator-session-2026-08-31-21`
- 状态：`blocked`（Windows 签名环境缺失）

## 改动摘要

- `playwright.config.ts`：移除错误 `webServer` 配置；改为 `INSIGHTFORGE_CDP_ENDPOINT` 或 `INSIGHTFORGE_APP_URL` 驱动。
- `package.json`：新增 `@playwright/test`、`@tauri-apps/playwright` 依赖；新增 `test:e2e` 脚本。
- `tsconfig.e2e.json`：E2E 专用 TypeScript 配置。
- `e2e/golden-path.spec.ts`：7 条用户路径测试选择器框架。
- `e2e/README.md`：记录自动化路径方案与 BLOCKED 条件。

## 验收标准 → 证据映射

| 验收标准 | 实现位置 | 可复跑命令/驱动 | 原始结果 | 状态 |
| --- | --- | --- | --- | --- |
| WebDriver E2E 覆盖 7 路径 | `e2e/golden-path.spec.ts` | `bun run test:e2e` | N/A | FAIL（未运行） |
| 签名打包版上通过 | `playwright.config.ts` | `INSIGHTFORGE_CDP_ENDPOINT=... bun run test:e2e` | N/A | FAIL（未运行） |
| 原始 WebDriver 日志、截图 | `playwright.config.ts` | `e2e-report/` | N/A | FAIL（未生成） |
| 不把安装包当 HTTP server | `playwright.config.ts` | `grep webServer playwright.config.ts` | 无匹配 | PASS |

## 已运行命令与原始结果

```text
# 以下命令因环境限制未执行
bun install --frozen-lockfile
bun run test:e2e:install
bun run test:e2e
```

## Golden Path 证据

- 驱动：`WSL 环境无法运行 Windows Tauri/WebView2；需 Windows 10/11 + WebView2 + 签名安装包`
- 原始日志：`N/A`
- 截图/录像：`N/A`
- 环境：`WSL（无法运行 Tauri 桌面应用或 Playwright WebDriver）`
- 未执行的真实步骤：`安装签名包 → 启用 WebView2 CDP → Playwright 连接 → 执行 7 条用户路径 → 收集日志/截图/版本信息`

## 风险、限制与失败会话

- **BLOCKED**：无 Windows 签名安装包、无 WebView2 CDP 自动化验证、无签名证书。
- 选择器基于代码阅读，未在实际 WebView2 渲染中验证。
- `@tauri-apps/playwright` adapter 对 Tauri 2 WebView2 的支持待验证。
- `INSIGHTFORGE_UPDATER_ENDPOINT` 需替换为实际发布地址（GitHub Releases 或静态 JSON）。

## 推荐下一步

1. **BLOCKED**：需 Windows 环境权限（实际物理机或 VM + WebView2 + 签名证书）。
2. 在 Windows 环境执行：
   - `bun install --frozen-lockfile && bun run test:e2e:install`
   - 配置 `INSIGHTFORGE_CDP_ENDPOINT` 或验证 `@tauri-apps/playwright` 连接路径
   - 运行 `bun run test:e2e` 并收集原始 WebDriver 日志
3. 验证通过后合并 `ticket/15-windows-e2e`。
