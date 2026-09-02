# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- 桌面端 Tauri 静态导出支持（`bun run desktop:export`）
- 知识库搜索页面（`/search`），支持碎片与成文的关键词检索
- 成文详情页（`/library/[id]`），可查看单篇成文并搜索相关碎片
- MCP 服务（`/api/mcp`），支持 Streamable HTTP，提供只读工具集
- AI 记忆机制：从已发布成文提议增量更新创作者身份档案
- LLM 设置卡片，支持在设置页配置 BYOK 参数
- 服务端 BYOK 代转路由（`/api/skeleton`、`/api/skeleton/expand`、`/api/discover`）
- 代转路由 IP 限流（20 次/分钟）+ SSRF 防护
- 数据库迁移脚本（`db:migrate`、`db:generate`、`db:push`）
- 索引-搜索时序契约：`sources_await_index_committed` 命令确保 FTS 提交后搜索才启用（C14）

### Changed
- 升级 Next.js 16 + React 19
- 统一降级语义：AI 不可用时前端回退本地行为（预置骨架 / 本地启发式）
- 身份档案从设置页独立卡片升级为可被 AI 记忆机制建议更新的模块

### Fixed
- 桌面端静态导出深链页样式丢失问题（注入 `<base href="/">`）
- 浏览器 BYOK 直连 CORS 拦截时的优雅降级

---

## [0.2.0] - 2026-08-09

### Added
- 创作者身份档案（`pointOfView / audience / voice / topics`）
- 设置页护栏阈值调整（AI 占比 / 展开次数 / 每日流入）
- AI 记忆机制：从成文观察并提议更新身份档案

### Changed
- 骨架生成与发现选题注入身份档案，让产出更「像你」

---

## [0.1.0] - 2026-08-09

### Added
- 基础闭环：碎片 / 空白 / 骨架 / 草稿 / 成文 / 回流
- 空白勘探（本地启发式 + AI 发现）
- 三栏写作画布（反代写护栏）
- 成文库（Markdown 导出）
- 双后端自动降级（Postgres / localStorage）
- BYOK OpenAI 兼容 API 接入
- 基础国际化（zh-CN / en-US）
