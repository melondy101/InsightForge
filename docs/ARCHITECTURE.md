# 织知 InsightForge — 架构概览（桌面版）

> 版本：目标 1.0.0（draft）；当前构建版本见 `tauri.conf.json` 的 `0.1.0`
> 最后更新：2026-08-30（2026-09-02 修订版本说明）
> 选型对比与理由：[docs/research/2026-08-29-desktop-architecture-options.md](./research/2026-08-29-desktop-architecture-options.md)
> 导出方案：[docs/research/2026-08-30-platform-export-options.md](./research/2026-08-30-platform-export-options.md)

---

## 1. 架构原则

- **本地优先**：知识库是用户自己的只读文件夹；SQLite 索引库可全量重建；关闭即退出，无后台守护。
- **最小权限**：React 只拿窄而有类型的业务命令；renderer 无任意文件系统、任意网络、任意 SQL 权限。
- **反代写边界**：AI 输出仅限动作白名单；出站仅限 `EgressBundle`；密钥只进 OS 凭据库。
- **无损 Markdown**：磁盘 `.md` 是唯一真相；编辑器与导出均为派生物。
- **单一写入者**：SQLite 由 Rust repository 独占，不经通用 SQL 插件暴露给前端。

## 2. 技术栈

| 层 | 技术 | 备注 |
| --- | --- | --- |
| 桌面壳 | Tauri 2 | 系统 WebView2 + bootstrapper；不捆绑 fixed runtime |
| 前端 | React + TypeScript + Vite | 无 Next.js / SSR / API routes |
| 编辑器 | CodeMirror 6 | 文档保持平面字符串，适合无损保存；藏于 `EditorAdapter` 后 |
| 本地核心 | Rust | 文件、并发、SQLite、平台 API |
| 数据 | SQLite + FTS5 | WAL + 单写队列；≥ 3.51.3（含 2026 WAL-reset 修复） |
| 密钥 | `keyring` → Windows Credential Manager | 未来 macOS Keychain / Linux Secret Service |
| 导出 | remark → rehype 管线（TS 侧） | `remark-parse → remark-gfm → remark-rehype → rehype-stringify` + sanitize |
| 打包/更新 | NSIS / MSI + Tauri updater | 公开发布时签名校验强制、不可关闭 |

## 3. 系统形状

```text
React / TypeScript / Vite
  ├─ Workspace UI（可调三栏，正文宽度下限 35%）
  ├─ Markdown editor adapter（CodeMirror 6）
  ├─ Evidence shelf（证据架）
  └─ Coach UI（仅显式「我卡住了」后出现）
                 │ typed commands + event channels
                 ▼
Rust application core
  ├─ sources      目录授权、只读边界、忽略规则
  ├─ indexer      扫描、监听、hash、解析、分块、增量事务
  ├─ search       本地关键词召回、排序、片段上下文
  ├─ drafts       Markdown 打开、原子保存、冲突检测
  ├─ coaching     三轮协议、接续条件、结构化响应校验
  ├─ ai_gateway   出站裁剪、OpenAI-compatible client、审计摘要
  ├─ secrets      OS credential-store adapter
  └─ repository   SQLite schema、迁移、备份/重建
                 │
                 ├─ App data/insightforge.sqlite
                 ├─ selected knowledge folders (read-only)
                 └─ selected drafts/output folder (read-write)
```

- 阻塞型工作（扫描/解析/hash/数据库写入）运行在专门 executor，不阻塞 UI 路径；必须带显式 cancellation token（已开始的 blocking work 不能靠丢弃 handle 停止）。
- 事件通道推送索引进度等状态给前端。

## 4. 前端 / Rust 边界

只注册业务命令，不放通用能力：

```
sources.add / list / pause / remove / sync / reconcile / rebuild_index / set_sync_interval
sources_await_index_committed   # 阻塞等待索引提交，返回 IndexStatus（含 searchable 布尔）
search.query / open_context
settings.get / set_ai / set_ai_enabled / set_api_key / clear_api_key / test_connection
ai.sample
drafts.ensure_directory / list / create / open / save / delete / resolve_conflict / cleanup
coach.start / respond / get_session / mark_helpful / archive
thoughts.upsert / list / list_by_draft / list_by_source / delete
references.create / list / adopt / mark_source_deleted
egress.build / audit.log / audit.list
diagnostics.snapshot / diagnostics.event_log   # 脱敏诊断快照 / 结构化事件日志（用户可触发复制/导出）
```

Tauri 2 的 permissions / scopes / capabilities 负责命令与窗口绑定；但**动态目录授权的领域规则**（知识库只读、草稿目录可写、symlink 不越界、路径不上传）必须在 Rust 业务服务内执行，不能只靠通用 FS scope。

## 5. 索引与检索

- **watcher 不是事实来源**。文件事件只负责尽快触发工作；启动补扫、手动同步、5/15/30 分钟间隔校验负责最终一致（跨平台 watcher 对网络文件系统和超大目录可能遗漏事件）。
- 增量流程：
  1. 规范化并验证授权根目录；默认不跟随逃出根目录的 symlink。
  2. 遍历允许的扩展名与 ignore globs，记录稳定文件标识（可用时）、规范路径、size、mtime、内容 hash。
  3. 仅对新增或 hash 改变的文件解析；Markdown 优先按标题，其次按段落与长度窗口分块。
  4. 单个 SQLite 事务内替换该文件的片段与 FTS 行；进度经事件通道推送。
  5. 删除文件退出未来检索；已被引用的片段保留不可变快照并标记来源不可用。
  6. 重命名优先按 file identity，其次按内容 hash + 时间窗口。
- FTS5 提供 `bm25()`、`highlight()`、`snippet()`；中文实测 trigram tokenizer，两字符查询回退受限线性扫描。

## 6. 草稿与冲突

- 打开时保存基线 `{file identity, content hash, mtime, size}`；每次保存先重读元数据，外部已变化 → 返回结构化 `Conflict`（diff 三选一：保留本地 / 采用外部 / 另存为），绝不静默覆盖。
- 正常保存：同目录临时文件写入 → flush → sync → 平台替换（Windows `ReplaceFileW`；Unix 同文件系统 rename）→ **成功后才更新数据库基线**。
- 集成测试覆盖断电、进程崩溃、锁占用、杀毒软件干扰。

## 7. AI 网关

- 每次远端调用前由 Rust 生成不可变 `EgressBundle`：选区/光标前后段落、标题/大纲、已引用素材、≤3 条检索片段、无本机路径的来源标题；UI 明示「将发送 N 个片段」。**只有该 bundle 可进入 HTTP client。**
- 响应必须解析为白名单动作 `question | evidence | tension | directions | micro_task`；疑似连续正文直接丢弃并替换为本地安全提示。
- BYOK 存 OS 凭据库（Tauri Stronghold 是加密 vault，不等同 OS 凭据库，不混用）。
- `ai_call_audit` 记录模型、时间、发送片段数量/ID、状态；不存敏感全文。

## 8. 索引-搜索时序契约

### 背景：Tauri 事件投递不可靠

Tauri 2 `handle.emit("sources://progress", ...)` 在开发模式 WebView2 通道中存在事件丢失窗口。Backfill 完成时的 `BackfillCompleted` 事件**无法可靠到达前端 listen**，导致前端仅依赖事件驱动的进度 UI 会出现状态不同步。

### Condvar 解决方案

引入 `JobDone` 信号（`Arc<Mutex<bool>> + Condvar`）：

- backfill 完成后，executor 线程内先 `running.remove(job_id)`，再 `mark_done()` → `notify_all()`
- `await_index_committed()` 若发现 source 仍在 `running` map，则 `Condvar::wait_while(|d| !*d)` 阻塞
- **零 sleep / 零 poll**：唤醒由 Condvar 精确触发，无时间猜测

### 公共命令

`Tauri command` `sources_await_index_committed(sourceId: String) -> IndexStatus`

```
阶段 0: 验证 source 存在（返回 SourceNotFound 若已删除）
阶段 1: 若在 running map，阻塞等待 Condvar 信号
阶段 2: 从 DB 读 file_count + fragment_count
返回: IndexStatus { source_id, file_count, fragment_count, searchable }
```

**契约保证**：当 `searchable = true` 时，FTS5 倒排索引已在 writer 线程 commit；`SearchService::query(source_id, …)` 必能命中已索引片段。

### 错误处理要点

- `spawn_backfill` 错误路径：先 `running.remove()` + 回滚，后 `mark_done()`，防止 `await_index_committed` 在清理前看到信号
- Condvar poison 处理：`lock().unwrap_or_else(|e| e.into_inner())` 保留原始 panic 上下文，允许等待继续
- `COUNT(*)` 在 SQLite 中永远 `>= 0`，`u64::try_from` 使用 `expect` 而非 `unwrap_or(0)`


## 9. 数据骨架

| 表 | 用途 |
| --- | --- |
| `knowledge_source` | 用户命名的根目录、启停、ignore、同步游标 |
| `source_file` | 相对路径、file identity、hash、mtime、状态（完整路径不进入 AI 数据） |
| `fragment` | 文件内定位、标题路径、文本、内容 hash |
| `fragment_fts_*` | FTS5 派生索引，可完全重建 |
| `draft` | 外部 Markdown 路径标识、保存基线、写作意图、布局 |
| `draft_reference` | 草稿位置、片段 ID、不可变引用快照、采用状态 |
| `coach_session` | 三轮状态、接续条件、摘要、帮助反馈 |
| `coach_turn` | 会话轮次与结构化动作；发布后默认折叠且不进检索 |
| `thought_card` | 作者确认的想法、素材与草稿位置关系 |
| `ai_call_audit` | 模型、时间、发送片段数量/ID、状态 |

首次运行建立全新 schema，**不做旧数据迁移**。FTS 表、扫描游标、缓存为派生数据；引用快照、思考卡、求助摘要、保存基线不是缓存，不随「重建索引」删除。

## 10. 导出管线

- 位于 React/TypeScript 一侧：`remark-parse → remark-gfm → remark-rehype → rehype-stringify`，输出 HTML 必须加 sanitize。不塞编辑器、不进 Rust。
- 两个目标：
  - `generic-html`：语义化、已消毒、含 `Clean Reading` 阅读排版（含 `<style>`）的完整 HTML 文件，供 WordPress 等 CMS。
  - `wechat-clipboard-html`：同一 Markdown AST，标签/属性白名单 + 内联样式（主题 CSS 计算内联，借鉴 doocs/md + Juice 模式），双写 `text/html` 与 `text/plain` 剪贴板。
- 首版一个固定主题 + 一个强调色；Markdown 内嵌 HTML 默认禁用；图片仅允许远程 `https` URL，不可访问时阻断导出并明确提示。

## 11. 打包、更新与测试

- 公开发布：签名 NSIS 或 MSI + 系统 WebView2 bootstrapper；updater 走静态 JSON / GitHub Releases（签名强制），不自建业务服务器。
- 当前个人自用基线：updater 配置和 fail-closed 注入已验证；没有签名安装包、公开 HTTPS endpoint 或安装升级 E2E，因此不能作为公开发布验证依据。
- 测试三层：
  1. Rust 单元/属性测试：分块、路径边界、ignore、hash 迁移、排名、三轮状态机、AI 出站裁剪。
  2. 临时目录 + SQLite 集成测试：新增/修改/删除/重命名、watch 漏事件后的 reconciliation、10k 文件基准、草稿冲突与失败恢复。
  3. Windows 桌面 E2E：目录选择、索引进度、证据卡、编辑保存、外部冲突、AI consent（WebDriver；纯浏览器 renderer 模式不能代替真实文件与安装测试）。
- CI（macOS/Linux 未支持期间）：至少编译 Rust core 并跑平台无关测试。

## 12. 平台支持状态

- **Windows 首发**；macOS / Linux 未承诺。正式宣称前必须在真实 WKWebView / WebKitGTK 上通过编辑器输入法、selection、clipboard、drag/drop 与 E2E —— 系统 WebView 差异是 Tauri 相对 Electron 最重要的长期代价。

## 13. 已知取舍

- 用 Rust 换取可靠的本地核心与更窄的安全边界（接受 Rust 学习与编译成本）。
- 用系统 WebView 换取更轻的分发（接受跨 OS 编辑器兼容测试义务）。
- 首版只有本地关键词检索，无 embedding / 向量库 / 后台守护。
- 用 Markdown source-of-truth 换取无损互操作，不承诺第一版 WYSIWYG。
- 删除旧应用数据、不维护迁移路径，换取全新 schema 与更少历史包袱。
- 回退条件：仅当 WebView2 编辑器兼容性问题不可接受且无法隔离时，才回退评估 Electron。
