# 织知 InsightForge

> 本地优先的桌面反代写写作工作台 —— AI 只递问题与证据，血肉由你亲手补。

> 桌面版重写已合入 `main`，目前处于 Windows 个人自用验证阶段。旧 Web 版已冻结，不再演进；需求、路线图和架构见 [docs/PRD.md](./docs/PRD.md)、[docs/ROADMAP.md](./docs/ROADMAP.md) 与 [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md)。

「织知」不替你写，而是做三件市面工具都不做的事：

1. **本地知识库主权** —— 知识库是你的本地文件夹；应用只读索引，源文件永不被修改。
2. **反代写陪练** —— 只问问题、指证据、点张力、给方向、拆微任务，绝不输出成段正文。
3. **出站可审计** —— 每次 AI 调用先生成最小出站包；BYOK 密钥存系统凭据库并留审计。

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

- **来源管理**：授权本地知识库目录（`.md` / `.txt`），配置忽略规则；来源目录对应用只读。
- **本地索引**：增量 FTS5 索引，支持中文检索；索引进度可见。
- **检索取证**：bm25 排序 + snippet 上下文；结果以证据卡呈现，可引用进草稿。
- **写作工作台**：可调三栏；CodeMirror 6 Markdown source editor；证据架侧栏。
- **草稿与冲突**：真实 `.md` 文件；原子保存；外部修改后返回结构化冲突，绝不静默覆盖。
- **陪练**：显式触发；三轮协议；AI 响应解析为白名单动作，疑似连续正文会被丢弃。
- **导出**：通用 HTML 和公众号剪贴板 HTML，均经 sanitization。

## 技术栈

- 桌面壳：Tauri 2（系统 WebView2，Windows 首发）
- 前端：React + TypeScript + Vite；i18n：react-i18next（zh-CN / en-US）
- 本地核心：Rust（`crates/core`，repository 独占 SQLite）
- 数据：SQLite + FTS5（WAL + 单一写入队列）

架构细节见 [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md)。

## 本地开发

```bash
bun install                  # 安装前端依赖
bun run tauri dev            # Tauri 开发模式（同时启动前端 dev server）
bun run gate                 # lint / tsc / test / build + Rust fmt / clippy / test / build
bun run tauri build          # 打包桌面应用
```

首次启动会在应用数据目录创建 `insightforge.sqlite` 并运行版本化迁移。

## 索引-搜索时序契约

为确保搜索执行时 FTS5 索引已经提交，Rust 提供 `sources_await_index_committed(sourceId: String) -> IndexStatus`。

1. 前端在来源变更后调用该命令。
2. 如果对应 backfill 尚在运行，Rust 通过 `JobDone` Condvar 同步等待，不使用 sleep 或轮询。
3. 返回 `IndexStatus` 后，前端仅在 `searchable=true` 时启用搜索。

`IndexStatus` 包含来源 ID、已索引文件数、片段数和 `searchable`；后者为 `true` 时，表示 FTS5 已写入 SQLite，搜索可命中已索引片段。

## 环境变量

桌面版不要求 `.env`。AI 密钥通过设置界面存入 Windows Credential Manager，不进入代码仓库。

| 设置项 | 说明 |
| --- | --- |
| AI Base URL | OpenAI 兼容 API base，默认 `https://api.openai.com/v1` |
| AI Model | 模型名，默认 `gpt-4o-mini` |
| AI Key | BYOK 密钥，存系统凭据库，应用内显示为已配置或未配置 |

## 开发者辅助：Agent Skills

仓库内置 [`mattpocock/skills`](https://github.com/mattpocock/skills)（`.agents/` 为源，`.claude/` 供 Claude Code）。
