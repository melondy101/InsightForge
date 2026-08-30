# 01 — 桌面工作区骨架与 SQLite 地基（含移除旧 Web 应用）

**What to build:** 开发者以开发模式启动桌面应用，能看到一个三栏占位的应用壳；首次启动在应用数据目录创建 SQLite 库并跑通版本化迁移；CI 编译 Rust core 并运行平台无关测试全绿。同一票据内移除旧 Next.js Web 应用代码——保留 docs/、CONTEXT.md、ADR、调研存档与 .scratch/，仓库从此以桌面工作区为唯一主线。

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] `tauri dev` 启动应用壳，三栏占位布局可见；前端为 React + TypeScript + Vite，无 Next.js / SSR / API routes
- [ ] 首次启动在应用数据目录创建 SQLite 库，迁移框架可执行版本化迁移；WAL + 单一写入队列 + SQLite ≥ 3.51.3（含 2026 WAL-reset 修复）在 repository 层固化
- [ ] 旧 Web 应用代码已移除：旧构建/测试脚本与依赖清理干净，文档、ADR、CONTEXT.md、调研存档完整保留
- [ ] CI 编译 Rust core 并运行平台无关测试，全绿
- [ ] 「提交前三绿」约定更新为新栈等价物（Rust 测试 + 前端类型检查 + lint）

## Comments
- 2026-08-30 Builder attempt 1：handoff 见 .scratch/desktop-rewrite/handoffs/01-handoff.md；commit 50667a0（移除旧 Web 应用）/ 5716f59（新前端工作区）/ 84dde72（Rust core 地基）/ edbb7ed（Tauri 2 壳建库）/ 2012921（门禁与文档）/ 6a129ba（lint 修复）+ handoff 提交（HEAD，见 git log 末条）。
