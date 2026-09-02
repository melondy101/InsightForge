# 织知 InsightForge

> 个人知识库驱动的**反代写**写作引擎 —— AI 只给骨架，血肉由你亲手补。

> ⚠️ **桌面版重写进行中**（2026-08-30 定稿）：项目正转向 Tauri 2 + Rust 本地核心的纯桌面应用，旧 Web 版（≤ 0.3.9）已冻结并于 01 号票移除全部代码，不再演进。桌面版需求与节奏见 [docs/PRD.md](./docs/PRD.md)、[docs/ROADMAP.md](./docs/ROADMAP.md)、[docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md)；实施票据见 [.scratch/desktop-rewrite/spec.md](./.scratch/desktop-rewrite/spec.md)。下文「页面」「环境变量」「API 路由」「MCP」「部署到 Vercel」等节为 Web 版（≤ 0.3.9）历史说明，仅作存档。

「织知」不替你写，而是做三件市面工具都不做的事：

1. **空白勘探** —— 告诉你知识库里「该写什么」（相邻概念都齐、却还没写过的空白点）。
2. **反代写约束** —— AI 只生成只读的提纲骨架 / 要点，绝不输出成段文字；写作画布实时统计「你的字数 vs AI 字数占比」，超阈值即告警。
3. **回流飞轮** —— 写完发布，成文回流进知识库，下次勘探更准。

> 📖 领域术语 / 共享语言见 [`CONTEXT.md`](./CONTEXT.md)。关键技术决策见 [`docs/adr/`](./docs/adr)。

## 主闭环

```
今日总览 → 空白勘探（导入存量内容 / 帮我找灵感）→ 选一个空白 → 生成骨架
        → 三栏写作画布补血肉 → 发布并回流 → 成文库（可导出 Markdown）
```

## 从哪来内容

- **导入存量内容**：在「空白勘探」页粘贴日记 / 摘抄 / 旧文，或上传 `.txt` / `.md`，按段落切成碎片入库并自动归簇。
- **快速采集**：随手记一条，自动归到最匹配的簇。
- **发现可写点（帮我找灵感）**：
  - 配了 AI Key → 走 AI，读你的碎片归纳主题、给出带理由的选题；
  - 没配 Key / AI 失败 → **自动回退**本地启发式（按碎片密度+新鲜度打分），App 始终可用。

## 页面

- `/` 今日总览：今日流入 / 待办空白 / 在写草稿（统计卡可点击跳转）+ 推荐先写 + 写作连续天数激励
- `/gaps` 空白勘探：碎片场 → 空白列表 → 详情；顶部有「导入存量内容」与「帮我找灵感」
- `/search` 知识库搜索：在碎片与成文中做关键词检索，结果可跳转回对应空白或成文
- `/write/[id]` 写作画布：三栏（AI 只读骨架 / 你的留白正文 / 相关碎片素材）+ 人工/AI 字数占比护栏 + 请求展开次数上限 + 发布回流
- `/library` 成文库：已发布成文 + 回流状态 + 人工字占比 + 复制/下载 Markdown；点击单篇进入 `/library/[id]` 详情
- `/settings` 设置：反代写护栏阈值（AI 占比上限 / 请求展开次数 / 每日碎片流入上限）+ 创作者身份档案 + AI 记忆机制

## 技术栈（桌面版）

- 桌面壳：Tauri 2（系统 WebView2，Windows 首发）
- 前端：React + TypeScript + Vite；i18n：`react-i18next`（zh-CN / en-US，默认中文）
- 本地核心：Rust（`crates/core`，repository 独占 SQLite）
- 数据：SQLite + FTS5（WAL + 单一写入队列；≥ 3.51.3）
- 详细决策见 [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md)

## 本地开发

```bash
bun install              # 安装前端依赖
bun run dev              # 仅前端（Vite dev server，http://localhost:1420）
bun run tauri dev        # 桌面应用开发模式（Vite + Tauri 壳）
bun run lint             # eslint
bun run test             # 运行 vitest
bun run build            # vite build -> dist/
bun run gate             # 提交前一键门禁（前端 lint/tsc/test/build + Rust fmt/clippy/test/build）
```

SQLite 数据库由 Rust repository 在首次启动时于应用数据目录自动创建（`insightforge.sqlite`）并执行版本化迁移，无独立迁移命令。

## 环境变量

复制 `.env.example` 为 `.env`（或在部署平台配置）：

| 变量 | 说明 |
| --- | --- |
| `OPENAI_API_KEY` | LLM 的 API Key。留空则 AI 回退到前端预置骨架 / 本地启发式发现 |
| `OPENAI_BASE_URL` | 兼容服务商 base，默认 `https://api.openai.com/v1`（DeepSeek 用 `https://api.deepseek.com/v1`） |
| `OPENAI_MODEL` | 模型名，默认 `gpt-4o-mini`（DeepSeek 用 `deepseek-chat`） |
| `DATABASE_URL` | Postgres 连接串（兼容 Neon / Eazo 托管 / Vercel Postgres / Supabase 等）。留空则数据走浏览器 `localStorage` |
| `MCP_TOKEN` | 启用 `/api/mcp` 知识库 MCP 服务的访问令牌。留空则 MCP 端点整体禁用（需同时配 `DATABASE_URL`） |
| `NEXT_PUBLIC_APP_TITLE` | 应用标题（可选） |
| `NEXT_PUBLIC_APP_DESCRIPTION` | 应用描述（可选） |

## API 路由

- `GET/PUT /api/kb`：知识库状态读写（数据库后端时的持久化）。GET 返回当前状态或初始化 seed；PUT 全量保存。
- `POST /api/skeleton`、`/api/skeleton/expand`：生成/展开骨架（只出提纲）。受 IP 限流（20 次/分钟）；无 Key 返回 503，前端回退预置骨架/提示。
- `POST /api/discover`：AI 发现可写点。受 IP 限流；无 Key 返回 503，客户端回退本地启发式。
- `GET/POST/DELETE /api/mcp`：知识库 MCP 服务（MCP Streamable HTTP）。需配 `MCP_TOKEN` + `DATABASE_URL`；未配令牌时返回 503。支持 `x-mcp-token` 与 `Authorization: Bearer` 两种认证。

> **CORS 降级说明**：三条 LLM 路由（`/api/skeleton`、`/api/skeleton/expand`、`/api/discover`）主路径是浏览器 BYOK 直连；当供应商 CORS 拦截时，前端会自动回退到服务端代转（key 用完即弃）。

## MCP：把知识库接入 AI 客户端

配好 `MCP_TOKEN` 与 `DATABASE_URL` 后，任意支持 MCP 的 AI 客户端（Claude Desktop / Cursor / 自定义 agent）可直接检索你的碎片与成文。当前为只读工具：`search_knowledge`、`list_writings`、`get_writing`、`list_gaps`。

```json
{
  "mcpServers": {
    "zhizhi": {
      "url": "https://<your-app>/api/mcp",
      "headers": { "x-mcp-token": "<MCP_TOKEN>" }
    }
  }
}
```

## 桌面端（Tauri）

```bash
bun run tauri dev        # 开发模式（Vite dev server + Tauri 2 壳）
bun run tauri build      # vite build -> dist/ 后由 Tauri 打包（NSIS/MSI）
```

首次启动会在应用数据目录创建 `insightforge.sqlite` 并跑版本化迁移。详见 `src-tauri/tauri.conf.json` 与 `docs/ARCHITECTURE.md`。

### 索引-搜索时序契约

为保证前端在用户调用 `search_query` 时 FTS5 索引已提交且可搜索，Rust 侧提供命令 `sources_await_index_committed(sourceId: String) -> IndexStatus`。

调用流程：

1. 前端 `SourcesView` 在 source 变更（add / sync / reconcile）后自动调用 `sources_await_index_committed`。
2. Rust 侧若对应 backfill 仍在运行，通过 `JobDone` Condvar 信号同步阻塞（无 sleep / 无 poll）。
3. 命令返回 `IndexStatus`（见下表）后，前端才启用搜索按钮。

`IndexStatus` 字段：

| 字段 | 类型 | 说明 |
|------|------|------|
| `source_id` | `String` | 来源 ID |
| `file_count` | `u64` | 已索引文件数（`source_file` 表 COUNT） |
| `fragment_count` | `u64` | 已建立片段数（`fragment` 表 COUNT，关联 `source_file`） |
| `searchable` | `bool` | 当 `fragment_count > 0` 时为 `true`，表示 `search_query` 必能命中 |

**保证**：`await_index_committed` 返回时 `searchable=true` → FTS5 倒排索引已写入 SQLite → `SearchService::query` 必命中已索引片段。


## 部署到 Vercel

1. 将本仓库导入 Vercel（框架自动识别为 Next.js）。
2. 在项目 Environment Variables 中按上表填入 `OPENAI_*`（可选）与 `DATABASE_URL`（可选）。
3. Deploy 即可。两类外部依赖都可缺省——缺省时 App 自动降级到本地可用状态。

## 开发者辅助：Agent Skills

仓库内置了 [`mattpocock/skills`](https://github.com/mattpocock/skills)（`.agents/` 为源，`.claude/` 供 Claude Code）。
在 Claude Code / Codex 等编码 agent 里可用 `/grill-me`、`/tdd`、`/code-review` 等；更新用 `npx skills update`。
