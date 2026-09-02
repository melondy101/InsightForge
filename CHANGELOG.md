# Changelog

本文件记录当前桌面版（0.1.0）的用户可见变更。旧 Web 版历史不再作为当前实现的变更记录。

## [Unreleased]

### Added

- Tauri 2 + Rust 本地核心、React/Vite 前端与 SQLite/FTS5 索引。
- 来源管理、关键词检索、真实 Markdown 草稿、引用快照、陪练、思考卡及 HTML/微信公众号剪贴板导出。
- `sources_await_index_committed`：等待 backfill 与 FTS 提交完成后才开放搜索，避免事件投递窗口造成的空结果。

### Fixed

- 草稿重开时的唯一约束冲突。
- 删除来源后引用快照的来源不可用状态和来源列表同步。

### Release status

- 已合入 `main`，面向 Windows 个人自用验证。
- 公开发布尚未验收：没有签名安装包、公开 HTTPS updater 或安装→升级 E2E 证据。
