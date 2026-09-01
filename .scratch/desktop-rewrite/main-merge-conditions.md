# Main 合并条件报告 · InsightForge 桌面重写

> 本文件由主控维护，记录 `integration/desktop-rewrite` 进入 `main` 的前置条件。
> 全部条件满足且取得用户明确授权前，不得合并、push、创建 PR 或发布。

## 当前状态（2026-09-01）

- **integration SHA**：`f5beb4e`（updater Finalizer 已合入）。
- **main SHA**：`baf7574`（尚未合并 desktop rewrite）。
- **集成工作树**：干净。
- **已确认代码门禁**：前端安装、类型检查、lint、114 项测试、构建；Rust fmt、clippy、92 项测试、构建均已通过。合并前应在目标提交上复跑并保存原始输出。

## 必须满足的合并条件

| 条件 | 状态 | 闭环标准 |
| --- | --- | --- |
| P0 代码缺陷 | 已解除 | `__demo__` 残留、updater 配置 schema、release 配置注入均已修复。 |
| 03/07 原生 GUI Golden Path | 进行中 | 在真实 Tauri 桌面端完成“搜索 → 固定 → 引用 → 草稿”并保存可复现证据。 |
| 14 签名安装与升级 | BLOCKED | 提供 Windows 签名证书、实际 updater 发布端点及安装升级实测证据。 |
| 15 Windows 安装包 E2E | BLOCKED | 提供签名安装包与可连接的 WebView2 自动化路径，完成端到端测试。 |
| 全量门禁 | 待合并前复跑 | 在将要合并的集成 SHA 上执行前端与 Rust 门禁，并记录原始输出。 |
| 用户授权 | 待授权 | 用户明确同意将 `integration/desktop-rewrite` 合入 `main`。 |

## 证据与交接

- 03/07：`.scratch/desktop-rewrite/handoffs/03-07-finalizer-handoff.md`
- 14：`.scratch/desktop-rewrite/handoffs/14-config-fix-handoff.md`、`.scratch/desktop-rewrite/handoffs/14-finalizer-updater-handoff.md`
- 15：`.scratch/desktop-rewrite/handoffs/15-handoff.md`
- 基础设施解除阻塞：`.scratch/desktop-rewrite/handoffs/gate-unblock-infra-handoff.md`
- 浏览器 Golden Path 记录：`.scratch/desktop-rewrite/golden-path/GOLDEN-PATH-EVIDENCE.md`

## 合并前的执行顺序

1. 关闭 03/07 原生 GUI 验收，补齐缺失 i18n 文案。
2. 准备签名证书、发布端点与 WebView2 自动化，关闭或正式维持 14/15 的 BLOCKED 票据。
3. 在 `f5beb4e` 或其后续最终集成 SHA 上复跑完整门禁。
4. 确认集成 worktree 干净并取得用户的明确合并授权。
5. 合并后再移除不再使用的 ticket worktree 及其本地分支。
