# 织知 InsightForge

> 本地优先的桌面反代写写作工作台 —— AI 只递问题与证据，血肉由你亲手补。

> ⚠️ **桌面版重写中**（2026-08-30 定稿）：项目正转向 Tauri 2 + Rust 本地核心的纯桌面应用。需求与节奏见 [docs/PRD.md](./docs/PRD.md)、[docs/ROADMAP.md](./docs/ROADMAP.md)；实施票据见 [.scratch/desktop-rewrite/spec.md](./.scratch/desktop-rewrite/spec.md)。

「织知」不替你写，而是做三件市面工具都不做的事：

1. **本地知识库主权** —— 知识库就是你自己的本地文件夹：应用只读索引、索引库可全量重建，源文件永不被修改；不把笔记搬进任何云端。
2. **反代写陪练** —— 只在显式说「我卡住了」时出现；只问问题、指证据、点张力、给方向、拆微任务，三轮之后把笔还给作者，绝不输出成段正文。
3. **出站可审计** —— 每次 AI 调用先由本地生成最小出站包并明示「将发送 N 个片段」；BYOK 密钥存系统凭据库；调用留审计。完整目录、完整文件、本机路径永远不出门。

> 📖 领域术语 / 共享语言见 [`CONTEXT.md`](./CONTEXT.md)。关键技术决策见 [`docs/adr/`](./docs/adr)。

## 主闭环

```
授权来源目录 → 后台索引（启动补扫 / 文件监听 / 间隔校验）
        → 检索取证（关键词 → 证据卡）
        → 打开草稿（真实 .md 文件）→ 引用证据 → 写作补血肉
        →（卡住时）陪练三轮：问题 / 证据 / 张力 / 方向 / 微任务
        → 导出发布：通用 HTML（WordPress 等 CMS）/ 公众号剪贴板 HTML
```

## 功能特性

- **来源管理**：授权本地知识库目录（`.md` / `.txt`），配置忽略规则；知识库目录对应用只读，任何写入路径在 Rust 侧被拒绝。
- **本地索引**：增量 FTS5 索引，支持 trigram 中文检索；索引进度可见；只读边界（symlink / path traversal）在 Rust 侧强制执行。
- **检索取证**：bm25 排序 + snippet 上下文；结果以证据卡呈现，可引用进草稿。
- **写作工作台**：可调三栏（编辑器居中，正文宽度下限 35%）；CodeMirror 6 Markdown source editor；证据架侧栏。
- **草稿与冲突**：真实 `.md` 文件；原子保存（Windows `ReplaceFileW`）；外部修改后保存返回结构化 `Conflict`，diff 三选一（保留本地 / 采用外部 / 另存为），绝不静默覆盖。
- **陪练**：显式触发「我卡住了」；三轮协议；AI 响应解析为白名单动作（`question | evidence | tension | directions | micro_task`）；疑似连续正文直接丢弃并替换为本地安全提示。
- **导出**：通用 HTML（语义化、已消毒、含阅读排版样式）；公众号剪贴板 HTML（标签/属性白名单、样式内联，双写 `text/html` 与 `text/plain`）。
- **签名安装与更新**：Windows 首发签名 NSIS / MSI + Tauri updater（签名强制、不可关闭）。

## 技术栈

- **桌面壳**：Tauri 2（系统 WebView2 + bootstrapper）
- **前端**：React + TypeScript + Vite（无 Next.js / SSR / API routes）
- **编辑器**：CodeMirror 6（`EditorAdapter` 适配层）
- **本地核心**：Rust（sources / indexer / search / drafts / coaching / ai_gateway / secrets / repository）
- **数据**：SQLite + FTS5（WAL + 单写队列）
- **密钥**：OS 凭据库（Windows Credential Manager）
- **导出**：`remark-parse → remark-gfm → remark-rehype → rehype-stringify`（含 sanitize）
- **测试**：Rust 单元/属性测试 · 临时目录 + SQLite 集成测试 · Windows 桌面 E2E（Playwright）
- **i18n**：react-i18next（zh-CN / en-US）

架构细节见 [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md)。

## 本地开发

```bash
bun install                  # 安装前端依赖
bun run tauri dev            # Tauri 开发模式（同时启动前端 dev server）
bun run lint                 # 代码检查
bun run test                 # 前端测试（vitest）
bun run build                # 前端构建（Vite → src-tauri/dist）
cargo test                   # Rust 测试
cargo build                  # Rust 构建
```

### 提交前三绿

在推送前，确保以下命令全部通过：

```bash
bun run test                 # 前端测试全过
bun run lint                 # 0 error / 0 warning
cargo test                   # Rust 测试全过
cargo build                  # Rust 构建通过
```

### 桌面端打包

```bash
bun run tauri build          # 打包桌面应用（签名配置见 src-tauri/tauri.conf.json）
```

> 桌面端前端通过 Vite 构建到 `src-tauri/dist`，不经 `desktop:export` 静态导出。

## 环境变量

桌面版无 `.env` 要求。AI 密钥通过应用设置界面存入系统凭据库（Windows Credential Manager），不入代码仓库。

| 设置项 | 说明 |
| --- | --- |
| AI Base URL | OpenAI 兼容 API base，默认 `https://api.openai.com/v1` |
| AI Model | 模型名，默认 `gpt-4o-mini` |
| AI Key | BYOK 密钥，存系统凭据库，应用内可见为「已配置」或「未配置」 |

## 开发者辅助：Agent Skills

仓库内置了 [`mattpocock/skills`](https://github.com/mattpocock/skills)（`.agents/` 为源，`.claude/` 供 Claude Code）。
在 Claude Code / Codex 等编码 agent 里可用 `/grill-me`、`/tdd`、`/code-review` 等；更新用 `npx skills update`。

---

## 旧版本文档存档（Web ≤ 0.3.9，已冻结）

> 以下内容描述的是已冻结的旧 Web 版，不再演进。桌面版从 0.1.0 重新计数，不迁移旧数据。

### 主闭环（旧版）

```
今日总览 → 空白勘探（导入存量内容 / 帮我找灵感）→ 选一个空白 → 生成骨架
        → 三栏写作画布补血肉 → 发布并回流 → 成文库（可导出 Markdown）
```

### 从哪来内容（旧版）

- **导入存量内容**：在「空白勘探」页粘贴日记 / 摘抄 / 旧文，或上传 `.txt` / `.md`，按段落切成碎片入库并自动归簇。
- **快速采集**：随手记一条，自动归到最匹配的簇。
- **发现可写点（帮我找灵感）**：
  - 配了 AI Key → 走 AI，读你的碎片归纳主题、给出带理由的选题；
  - 没配 Key / AI 失败 → **自动回退**本地启发式（按碎片密度+新鲜度打分），App 始终可用。

### 页面（旧版）

- `/` 今日总览：今日流入 / 待办空白 / 在写草稿（统计卡可点击跳转）+ 推荐先写 + 写作连续天数激励
- `/gaps` 空白勘探：碎片场 → 空白列表 → 详情；顶部有「导入存量内容」与「帮我找灵感」
- `/search` 知识库搜索：在碎片与成文中做关键词检索，结果可跳转回对应空白或成文
- `/write/[id]` 写作画布：三栏（AI 只读骨架 / 你的留白正文 / 相关碎片素材）+ 人工/AI 字数占比护栏 + 请求展开次数上限 + 发布回流
- `/library` 成文库：已发布成文 + 回流状态 + 人工字占比 + 复制/下载 Markdown；点击单篇进入 `/library/[id]` 详情
- `/settings` 设置：反代写护栏阈值（AI 占比上限 / 请求展开次数 / 每日碎片流入上限）+ 创作者身份档案 + AI 记忆机制

### 技术栈（旧版）

- Next.js（App Router）+ TypeScript + Tailwind CSS v4 + shadcn/ui
- **数据（双后端，自动降级）**：
  - 配了 `DATABASE_URL`（Postgres，兼容 Neon / Eazo 托管）→ 走数据库（Drizzle ORM，服务端 `/api/kb`）；
  - 未配 → 回退浏览器 `localStorage`（内置示例知识库，无需登录即可体验）。
- **AI（BYOK，自动降级）**：直连 **OpenAI 兼容 API**；未配置密钥时自动回退到前端预置骨架 / 本地启发式发现。

### 本地开发（旧版）

```bash
bun run dev              # 启动开发服务器 http://localhost:3000
bun run build            # Web 版构建（SSR + API routes）
bun run desktop:export   # Tauri 桌面端静态导出 -> out/
```

### 环境变量（旧版）

| 变量 | 说明 |
| --- | --- |
| `OPENAI_API_KEY` | LLM 的 API Key。留空则 AI 回退到前端预置骨架 / 本地启发式发现 |
| `OPENAI_BASE_URL` | 兼容服务商 base，默认 `https://api.openai.com/v1` |
| `OPENAI_MODEL` | 模型名，默认 `gpt-4o-mini` |
| `DATABASE_URL` | Postgres 连接串（兼容 Neon / Eazo 托管 / Vercel Postgres / Supabase 等）。留空则数据走浏览器 `localStorage` |
| `MCP_TOKEN` | 启用 `/api/mcp` 知识库 MCP 服务的访问令牌。留空则 MCP 端点整体禁用（需同时配 `DATABASE_URL`） |
| `NEXT_PUBLIC_APP_TITLE` | 应用标题（可选） |
| `NEXT_PUBLIC_APP_DESCRIPTION` | 应用描述（可选） |

### API 路由（旧版）

- `GET/PUT /api/kb`：知识库状态读写（数据库后端时的持久化）。GET 返回当前状态或初始化 seed；PUT 全量保存。
- `POST /api/skeleton`、`/api/skeleton/expand`：生成/展开骨架（只出提纲）。受 IP 限流（20 次/分钟）；无 Key 返回 503，前端回退预置骨架/提示。
- `POST /api/discover`：AI 发现可写点。受 IP 限流；无 Key 返回 503，客户端回退本地启发式。
- `GET/POST/DELETE /api/mcp`：知识库 MCP 服务（MCP Streamable HTTP）。需配 `MCP_TOKEN` + `DATABASE_URL`；未配令牌时返回 503。

### MCP 配置（旧版）

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
