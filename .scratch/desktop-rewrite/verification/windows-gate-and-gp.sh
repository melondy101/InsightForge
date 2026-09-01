#!/usr/bin/env bash
# InsightForge Windows 验证脚本
# 在 Windows 环境执行：Git Bash / WSL / CI
# 前置：安装 bun、cargo、rustc、Playwright

set -euo pipefail

INTEGRATION_WT="D:/Develop/wt/InsightForge/_integration"
REPORT="/mnt/d/Develop/InsightForge/.scratch/desktop-rewrite/verification/evidence-$(date +%Y%m%d-%H%M%S).txt"

echo "=== InsightForge Desktop Rewrite Verification ===" | tee "$REPORT"
echo "Integration worktree: $INTEGRATION_WT" | tee -a "$REPORT"
echo "Date: $(date)" | tee -a "$REPORT"
echo "" | tee -a "$REPORT"

# 1. 工作树完整性
echo "--- 1. Worktree integrity ---" | tee -a "$REPORT"
cd "$INTEGRATION_WT"
echo "HEAD: $(git rev-parse HEAD)" | tee -a "$REPORT"
echo "Status:" | tee -a "$REPORT"
git status --short | tee -a "$REPORT"
echo "" | tee -a "$REPORT"

# 2. 关键代码检查
echo "--- 2. Code checks ---" | tee -a "$REPORT"
echo "__demo__ check:" | tee -a "$REPORT"
(grep -r "__demo__" src/ || echo "PASS: no __demo__ found") | tee -a "$REPORT"
echo "" | tee -a "$REPORT"

echo "YOUR_PUBLIC_KEY check:" | tee -a "$REPORT"
(grep -r "YOUR_PUBLIC_KEY" src-tauri/ || echo "PASS: no placeholder pubkey") | tee -a "$REPORT"
echo "" | tee -a "$REPORT"

echo "webServer check (should be empty):" | tee -a "$REPORT"
(grep -r "webServer" playwright.config.ts || echo "PASS: no webServer misconfiguration") | tee -a "$REPORT"
echo "" | tee -a "$REPORT"

# 3. 前端门禁
echo "--- 3. Frontend gates ---" | tee -a "$REPORT"
cd "$INTEGRATION_WT"

echo "bun install --frozen-lockfile:" | tee -a "$REPORT"
bun install --frozen-lockfile 2>&1 | tee -a "$REPORT"
echo "Exit: $?" | tee -a "$REPORT"
echo "" | tee -a "$REPORT"

echo "bun run lint:" | tee -a "$REPORT"
bun run lint 2>&1 | tee -a "$REPORT"
echo "Exit: $?" | tee -a "$REPORT"
echo "" | tee -a "$REPORT"

echo "bun run typecheck:" | tee -a "$REPORT"
bun run typecheck 2>&1 | tee -a "$REPORT"
echo "Exit: $?" | tee -a "$REPORT"
echo "" | tee -a "$REPORT"

echo "bun run test:" | tee -a "$REPORT"
bun run test 2>&1 | tee -a "$REPORT"
echo "Exit: $?" | tee -a "$REPORT"
echo "" | tee -a "$REPORT"

echo "bun run build:" | tee -a "$REPORT"
bun run build 2>&1 | tee -a "$REPORT"
echo "Exit: $?" | tee -a "$REPORT"
echo "" | tee -a "$REPORT"

# 4. Rust 门禁
echo "--- 4. Rust gates ---" | tee -a "$REPORT"
cd "$INTEGRATION_WT"

echo "cargo fmt --check:" | tee -a "$REPORT"
cargo fmt --check 2>&1 | tee -a "$REPORT"
echo "Exit: $?" | tee -a "$REPORT"
echo "" | tee -a "$REPORT"

echo "cargo clippy --workspace --all-targets -- -D warnings:" | tee -a "$REPORT"
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tee -a "$REPORT"
echo "Exit: $?" | tee -a "$REPORT"
echo "" | tee -a "$REPORT"

echo "cargo test --workspace:" | tee -a "$REPORT"
cargo test --workspace 2>&1 | tee -a "$REPORT"
echo "Exit: $?" | tee -a "$REPORT"
echo "" | tee -a "$REPORT"

echo "cargo build --workspace:" | tee -a "$REPORT"
cargo build --workspace 2>&1 | tee -a "$REPORT"
echo "Exit: $?" | tee -a "$REPORT"
echo "" | tee -a "$REPORT"

# 5. 03/07 GUI 验证占位（需手动执行）
echo "--- 5. 03/07 GUI Golden Path (MANUAL) ---" | tee -a "$REPORT"
cat << 'MANUAL' | tee -a "$REPORT"
手动步骤：
1. 在 Windows 环境运行: cd $INTEGRATION_WT && bun run tauri dev
2. 等待应用启动，截图保存为: verification/gp-03-07-01-workspace.png
3. 点击「来源管理」，添加一个本地 Markdown 目录，等待索引完成
4. 截图: verification/gp-03-07-02-sources-added.png
5. 点击「检索」，输入关键词，执行搜索
6. 截图: verification/gp-03-07-03-search-results.png
7. 点击证据卡的「打开上下文」，验证上下文显示
8. 点击「草稿」，新建/打开一个草稿
9. 在草稿中引用证据卡（需 07 接线完成）
10. 点击「引用」视图，验证引用列表可见
11. 截图: verification/gp-03-07-04-references.png
12. 关闭应用
MANUAL
echo "" | tee -a "$REPORT"

# 6. 14 签名验证占位（需手动执行）
echo "--- 6. 14 Updater Golden Path (MANUAL) ---" | tee -a "$REPORT"
cat << 'MANUAL' | tee -a "$REPORT"
手动步骤：
1. 在 Windows 环境配置环境变量:
   - INSIGHTFORGE_UPDATER_PUBKEY=<真实公钥>
   - INSIGHTFORGE_UPDATER_ENDPOINT=https://updater.insightforge.app/{{target}}/{{current_version}}
   - INSIGHTFORGE_CERT_THUMBPRINT=<证书指纹>
2. 运行: cd $INTEGRATION_WT && bun run tauri build
3. 对产物进行签名
4. 安装签名后的安装包
5. 触发更新检查，验证签名校验流程
6. 截图保存原始证据
MANUAL
echo "" | tee -a "$REPORT"

# 7. 15 E2E 执行
echo "--- 7. 15 E2E ---" | tee -a "$REPORT"
cd "$INTEGRATION_WT"
echo "Playwright install:" | tee -a "$REPORT"
bun run test:e2e:install 2>&1 | tee -a "$REPORT"
echo "Exit: $?" | tee -a "$REPORT"
echo "" | tee -a "$REPORT"

echo "Playwright test (headed, local):" | tee -a "$REPORT"
bun run test:e2e:headed 2>&1 | tee -a "$REPORT"
echo "Exit: $?" | tee -a "$REPORT"
echo "" | tee -a "$REPORT"

echo "=== Verification complete ===" | tee -a "$REPORT"
echo "Report saved to: $REPORT" | tee -a "$REPORT"
