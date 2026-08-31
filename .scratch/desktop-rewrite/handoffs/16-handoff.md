# Handoff · Ticket 16 — Contribution Docs Fix

- Issue：`.scratch/desktop-rewrite/issues/16-contribution-docs-fix.md`
- Baseline SHA：a6db2d2
- Branch：ticket/16-contribution-docs-fix
- Worktree：D:\Develop\wt\InsightForge\16-contribution-docs-fix
- Role / attempt：Builder / attempt 1
- 失败会话 ID（如有）：

## 改动摘要

- 重写 `CONTRIBUTING.md`：移除 `desktop:export`、`./node_modules/.bin/tsc --noEmit`、Next.js/SSR/"use client" 引用；添加 Rust 门禁节（`cargo fmt/clippy/test/build`）；桌面端节改为 Tauri 2 命令
- `README.md`：未作结构性修改——环境变量、API 路由、MCP、Vercel 等节已在 README 第 5 行明确标记为 "Web 版（≤ 0.3.9）历史说明，仅作存档"；按约束 "不改变 README 中明确标记为历史存档的内容" 保留不动
- 提交 `29780ae`："docs: fix CONTRIBUTING.md for Tauri 2 + Vite + Rust"

## 验收标准 → 证据映射

| 验收标准 | 实现位置 | 可复核命令/测试 | 原始结果或证据路径 |
|---|---|---|---|
| CONTRIBUTING.md 不再推荐 desktop:export | CONTRIBUTING.md（全文） | `rg "desktop:export" CONTRIBUTING.md` | 无匹配（exit code 1，No matches） |
| 命令与当前 package.json 一致 | CONTRIBUTING.md 第 1–32 行 | 对照 package.json scripts | `dev=vite`, `build=vite build`, `tauri=tauri`, `lint=eslint .`, `typecheck=tsc --noEmit`, `test=vitest run`, `gate=...` |
| 不再将 Next.js/SSR/API routes 作为现行架构 | CONTRIBUTING.md（全文） | `rg "next dev|next build|use client|服务端 AI 客户端" CONTRIBUTING.md` | 无匹配 |
| 文档链接可静态核对 | CONTRIBUTING.md 第 39, 45, 106 行 | `rg "docs/|CONTEXT.md|AGENTS.md" CONTRIBUTING.md` | 链接保留 |

## 运行过的命令与原始结果

1. `cd "D:\Develop\wt\InsightForge\16-contribution-docs-fix" && bun install` — exit code `0`
   ```
   bun install v1.3.11 (af24e281)
   + @tauri-apps/cli@2.11.4
   ...
   205 packages installed [5.69s]
   ```

2. `bun run lint` — exit code `0`
   ```
   $ eslint .
   ```

3. `rg -n "desktop:export|next dev|next build|api/|MCP_TOKEN|DATABASE_URL|OPENAI_API_KEY" CONTRIBUTING.md` — exit code `1`
   ```
   No matches found
   ```

4. `rg -n "desktop:export|next dev|next build|api/|MCP_TOKEN|DATABASE_URL|OPENAI_API_KEY" README.md` — exit code `0`
   ```
   README.md:67: `OPENAI_API_KEY` | LLM 的 API Key...
   README.md:70: `DATABASE_URL` | Postgres 连接串...
   README.md:71: `MCP_TOKEN` | 启用 `/api/mcp`...
   README.md:77: `GET/PUT /api/kb`：知识库状态读写...
   ...（共 13 处，全部位于 README 第 5 行声明的历史存档节内）
   ```

5. `git add CONTRIBUTING.md && git commit -m "..."` — exit code `0`
   ```
   [ticket/16-contribution-docs-fix 29780ae] docs: fix CONTRIBUTING.md for Tauri 2 + Vite + Rust
    1 file changed, 35 insertions(+), 18 deletions(-)
   ```

## Golden Path / 手动验证

```bash
cd D:\Develop\wt\InsightForge\16-contribution-docs-fix
# 验证 CONTRIBUTING 无旧引用
rg -n "desktop:export|node_modules/\.bin/tsc|服务端 AI 客户端" CONTRIBUTING.md
# 验证 README 历史引用已被标记
sed -n '1,10p' README.md
# 验证 lint 通过
bun run lint
# 查看提交
git show --stat HEAD
```

## 风险、限制与已知问题

- **README 历史节未动**：环境变量、API 路由、MCP、Vercel 等节仍包含过时内容，但因 README 第 5 行已将其声明为 "Web 版（≤ 0.3.9）历史说明，仅作存档"，按约束未修改。后续若团队决定删除这些历史节，可单独出票处理。
- **缺失的 issue 文件**：`.scratch/desktop-rewrite/issues/16-contribution-docs-fix.md` 在 worktree 中不存在（仅 01–15 号票存在）。本 handoff 依据用户消息中提供的票据上下文执行。

## 推荐下一步

1. 如需清理 README 历史节，开新票处理（建议在桌面版上线后执行）
2. 考虑将 `docs/ARCHITECTURE.md` 的 Tauri 2 架构图同步到 CONTRIBUTING.md 作为参考链接
3. `npm`/`pnpm` 用户文档：当前 CONTRIBUTING.md 仅面向 `bun`，如需支持其他包管理器可补充说明
