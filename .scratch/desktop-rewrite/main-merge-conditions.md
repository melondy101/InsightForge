# Main 合并条件报告 · InsightForge 桌面重写

> 本文件由主控维护，记录 `integration/desktop-rewrite` 进入 `main` 的前置条件。
> 全部条件满足且取得用户明确授权前，不得合并、push、创建 PR 或发布。

## 当前状态（2026-09-01）

- **integration SHA**：`761297a`（C14 索引-搜索时序契约及相关文档已合入）。
- **main SHA**：`9df5844`（尚未合并 desktop rewrite）。
- **集成工作树**：干净。
- **已确认代码门禁**：前端安装、类型检查、lint、119 项测试、构建；Rust fmt、clippy、105 项测试、构建均已通过。合并前应在目标提交上复跑并保存原始输出。

## 必须满足的合并条件

| 条件 | 状态 | 闭环标准 |
| --- | --- | --- |
| P0 代码缺陷 | 已解除 | `__demo__` 残留、updater 配置 schema、release 配置注入、C-13 P1 drafts UNIQUE 已修复。 |
| C-13-Fix 验收 | 已闭环（5/7 PASS，2/7 PASS pending CDP） | P1 drafts.rs open() UNIQUE、P2 sources_remove→mark_source_deleted、P2 SourcesView 实时刷新、P2 UTF-8 safe two-char fallback 全过；CDP Golden Path 待真实桌面环境复验。详见 |
| 03/07 原生 GUI Golden Path | 进行中 | C-13 修复已在 integration 落地，但完整复验（添加→创建→搜索→固定→引用→删除→快照保留）需在真实 Tauri 桌面端重跑 WebView2 CDP 驱动。 |
| 14 签名安装与升级 | BLOCKED | 提供 Windows 签名证书、实际 updater 发布端点及安装升级实测证据。 |
| 15 Windows 安装包 E2E | BLOCKED | 提供签名安装包与可连接的 WebView2 自动化路径，完成端到端测试。 |
| 全量门禁 | 待合并前复跑 | 在将要合并的集成 SHA 上执行前端与 Rust 门禁，并记录原始输出。 |
| 用户授权 | 待授权 | 用户明确同意将 `integration/desktop-rewrite` 合入 `main`。 |

## 证据与交接

- C-13-Fix：`.scratch/desktop-rewrite/handoffs/c-13-fix-handoff.md`（Builder + Reviewer PASS）
- 03/07：`.scratch/desktop-rewrite/handoffs/03-07-finalizer-handoff.md`、`.scratch/desktop-rewrite/handoffs/03-07-native-gui-closure-handoff.md`
- 14：`.scratch/desktop-rewrite/handoffs/14-config-fix-handoff.md`、`.scratch/desktop-rewrite/handoffs/14-finalizer-updater-handoff.md`
- 15：`.scratch/desktop-rewrite/handoffs/15-handoff.md`
- 基础设施解除阻塞：`.scratch/desktop-rewrite/handoffs/gate-unblock-infra-handoff.md`
- 浏览器 Golden Path 记录：`.scratch/desktop-rewrite/golden-path/GOLDEN-PATH-EVIDENCE.md`
- WebView2 CDP 证据：`.scratch/desktop-rewrite/golden-path/native-gui-cdp/`（C-13 修复需在新 fixture 上重跑）

## 合并前的执行顺序

1. 在真实 Tauri 桌面环境复跑 WebView2 CDP Golden Path（C-13 + 03/07），生成新 JSON/log/截图。
2. 准备签名证书、发布端点与 WebView2 自动化，关闭或正式维持 14/15 的 BLOCKED 票据。
3. 在 `78fe279` 或其后续最终集成 SHA 上复跑完整门禁并保存原始输出。
4. 确认集成 worktree 干净并取得用户的明确合并授权。
5. 合并 `integration/desktop-rewrite` → `main` 后复跑门禁，再 push main。
6. 合并后再移除不再使用的 ticket worktree 及其本地分支。
