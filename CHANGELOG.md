# Changelog

本文件记录当前桌面版（0.1.0）的用户可见变更。旧 Web 版历史不再作为当前实现的变更记录。

## [Unreleased]

### Added

- Tauri 2 + Rust 本地核心、React/Vite 前端与 SQLite/FTS5 索引。
- 来源管理、关键词检索、真实 Markdown 草稿、引用快照、陪练、思考卡及 HTML/微信公众号剪贴板导出。
- `sources_await_index_committed`：等待 backfill 与 FTS 提交完成后才开放搜索，避免事件投递窗口造成的空结果。
- 草稿冲突三选一（保留本地 / 采用外部 / 另存为），外部修改不再静默覆盖。
- 孤儿草稿自动登记：`list()` 发现磁盘 `.md` 即登记，不再静默丢弃。
- 原子保存临时文件唯一命名 + 启动/列表时清理残留，降低崩溃或杀软干扰下的残留风险。
- 数据库打开完整性校验：损坏库备份为 `.corrupt.bak` 后重建，应用可安全降级而非崩溃。
- 诊断视图：脱敏诊断快照（版本/平台/SQLite 版本/迁移/完整性/表计数，不含密钥与正文）与结构化事件日志，支持复制/导出，便于求助。
- E2E 黄金路径规格（7 条主路径，受 WebView2 探针门控为 BLOCKED，不伪 PASSED）与 Windows 手动验证脚本。
- 发布 Runbook 与 fail-closed 构建（缺失签名/端点/公钥时 release 构建明确失败；dev 不受影响）。

### Fixed

- 草稿重开时的唯一约束冲突。
- 删除来源后引用快照的来源不可用状态和来源列表同步。

### Release status

- 已合入 `main`，面向 Windows 个人自用验证。
- 公开发布尚未验收：没有签名安装包、公开 HTTPS updater 或安装→升级 E2E 证据。
