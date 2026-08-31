# 16 — 修复桌面贡献与质量门禁文档

**What to build:** 修复 01 遗留的失效贡献文档，使 CONTRIBUTING.md 和 README.md 中与开发流程、构建命令、质量门禁、架构说明相关的内容与当前实际桌面版（Tauri 2 + Vite + Rust）一致。

**Status:** merged — merge commit `29780ae` into `integration/desktop-rewrite`

**Blocked by:** None — 01 已合并。

## 验收标准

- 不再推荐已删除的 `desktop:export`；
- 开发、构建、门禁命令与当前 `package.json`/Cargo workspace 一致；
- 不再将不存在的 Next.js、SSR、API routes、旧服务端 AI 客户端作为现行架构；
- 文档链接和命令可静态核对。

## 允许范围

- `CONTRIBUTING.md`
- `README.md`（仅同步未标明"历史存档"的开发命令节）

## 禁止范围

- 不改 `AGENTS.md` 的产品规则；
- 不改变 README 中明确标记为历史存档的内容；
- 不改依赖。

## 推荐验证

- `rg` 查找旧命令残留；
- `bun run lint`；
- 主控核对 package scripts。
