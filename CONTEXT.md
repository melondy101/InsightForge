# CONTEXT.md — 织知 InsightForge 领域词典（Ubiquitous Language）

> 这份文档是人和 agent 共用的「共享语言」。代码里的变量/函数/命令命名、对话里的措辞，都应该从这里的术语派生。
> 一句话讲清产品：**本地优先的桌面反代写写作工作台——你的知识库在你自己的文件夹里，AI 只递问题与证据，血肉由你亲手补。**
> 适用范围：桌面版（Tauri 2 + Rust 核心）。旧 Web 版术语见文末对照表。

---

## 一、核心工作流（一句话记住产品）

```
来源 Source → 索引 Index → 取证 Evidence → 草稿 Draft → 陪练 Coach → 导出 Export
```

产品做三件市面工具不做的事：

1. **知识库主权**：知识库就是用户自己的本地文件夹——应用只读索引、索引库可全量重建，源文件永不被修改。
2. **反代写陪练**：只在「我卡住了」后出现，只输出五类结构化动作，三轮之后把笔还给作者，绝不写成段正文。
3. **出站可审计**：每次 AI 调用只发送最小出站包并事先明示；BYOK 存系统凭据库；调用留审计。

> 首版是单向工作台，不是回流飞轮：知识库只读，成文不回写。「把成文目录加为来源」是未来选项（见 ROADMAP）。

---

## 二、领域名词（Nouns）

| 术语（中） | 代码符号 | 定义 | 关键点 |
| --- | --- | --- | --- |
| **来源** | `KnowledgeSource` | 用户授权的本地根目录：命名、启停、ignore globs、同步游标。 | 对应用**严格只读**；完整本机路径永不进入 AI 数据。 |
| **源文件** | `SourceFile` | 来源下的一个文件：相对路径、file identity、内容 hash、mtime、状态。 | 删除后退出检索；被引用片段保留快照并标记「来源不可用」。 |
| **片段** | `Fragment` | 索引器把源文件切出的检索单元：文件内定位、标题路径、文本、内容 hash。 | Markdown 优先按标题、其次按段落与长度窗口分块。（旧版含义=粘贴的笔记，已废弃，见附录） |
| **证据卡** | `EvidenceCard` | 检索命中的呈现：片段文本 + 来源标题（无本机路径）+ bm25 排名 + snippet 上下文。 | 可固定在证据架上并引用进草稿。 |
| **证据架** | `EvidenceShelf` | 工作台侧栏中存放证据卡的区域。 | 引用即写入 `draft_reference`。 |
| **草稿** | `Draft` | 真实存在于授权输出目录的 `.md` 文件。 | 数据库只存元数据：路径标识、保存基线、写作意图、布局。草稿即成文，无发布快照表。 |
| **保存基线** | `SaveBaseline` | `{file identity, content hash, mtime, size}`。 | 每次保存前重读比对；替换成功后才更新。 |
| **冲突** | `Conflict` | 外部修改与本地版本的分歧。 | 呈现 diff，三选一：保留本地 / 采用外部 / 另存为；**绝不静默覆盖**。 |
| **引用快照** | `DraftReference` | 草稿位置 ↔ 片段 ID + 不可变引用快照 + 采用状态。 | 来源不可用时快照仍在，成文不受影响。 |
| **陪练** | `Coach` | 显式「我卡住了」后出现的 AI 会话，受三轮协议约束。 | 轮次存 `coach_session` / `coach_turn`；发布后默认折叠且不进检索。 |
| **陪练动作** | `CoachAction` | 白名单：`question \| evidence \| tension \| directions \| micro_task`。 | 疑似连续正文的输出直接丢弃并替换为本地安全提示。 |
| **出站包** | `EgressBundle` | 每次 AI 调用前由 Rust 生成的不可变载荷：选区/光标段落、标题/大纲、已引用素材、≤3 条命中片段、无本机路径的来源标题。 | UI 明示「将发送 N 个片段」；只有它可进入 HTTP client。 |
| **思考卡** | `ThoughtCard` | 作者确认的「想法 ↔ 素材 ↔ 草稿位置」关系。 | 作者资产，不随索引重建删除。 |
| **凭据** | `Secret` | BYOK API Key。 | 存 OS 凭据库（Windows Credential Manager；未来 Keychain / Secret Service）。 |
| **审计** | `AiCallAudit` | 模型、时间、发送片段数量/ID、状态。 | 不重复保存敏感全文。 |
| **派生数据** | `Derived` | FTS 表、扫描游标、缓存。 | 可全量重建；**非派生**（不可随重建删除）：引用快照、思考卡、求助摘要、保存基线。 |
| **索引状态** | `IndexStatus` | 等待索引提交完成时的返回结构：来源 ID、已索引文件数、已建立片段数、是否可搜索。 | `searchable = fragment_count > 0` 时为 `true`，`search_query` 必能命中。 |
| **完成信号** | `JobDone` | `Arc<Mutex<bool>> + Condvar`，backfill 完成后 notify_all 等待者。 | 零 sleep / 零 poll；解决 Tauri dev 模式下 `BackfillCompleted` 事件投递丢失问题。 |

---

## 三、领域动作（Verbs → Tauri 命令）

| 术语（中） | 命令 / 符号 | 含义 |
| --- | --- | --- |
| **添加/暂停/移除/同步来源** | `sources.add / pause / remove / sync` | 授权目录并命名；移除只删索引记录，不动源文件。 |
| **等待索引提交** | `sources_await_index_committed` | 阻塞等待 backfill 完成 + FTS commit，返回 `IndexStatus`；前端据此 gating 搜索按钮。 |
| **检索** | `search.query` | FTS5 bm25 排序 + snippet 上下文；中文 trigram，两字符回退受限线性扫描。 |
| **打开上下文** | `search.open_context` | 从证据卡跳到来源文件上下文。 |
| **补扫** | backfill（indexer 内部） | 启动补扫、手动同步、5/15/30 分钟间隔校验负责最终一致；watcher 事件只负责尽快触发。 |
| **打开/保存草稿** | `drafts.open / save` | 原子保存：临时文件 → flush → sync → 平台替换（Windows `ReplaceFileW`，Unix rename）。 |
| **解决冲突** | `drafts.resolve_conflict` | diff 三选一，绝不静默覆盖。 |
| **引用证据** | cite（写入 `draft_reference`） | 生成不可变引用快照。 |
| **开始/回应/接续/反馈陪练** | `coach.start / respond / resume / mark_helpful` | 三轮协议；结束把笔还给作者。 |
| **确认思考卡** | `thoughts.confirm / update` | 沉淀想法-素材-草稿位置关系。 |
| **导出** | `export` | `generic-html`（完整 HTML 文件）\| `wechat-clipboard-html`（内联样式片段，双写剪贴板）。 |
| **诊断导出** | `diagnostics_snapshot` / `diagnostics_event_log` | 用户可随时触发脱敏诊断快照与结构化事件日志（不含密钥/草稿正文/文件路径），复制或导出用于求助。 |

---

## 四、五条铁律

1. **知识库只读**：源文件永不被应用修改；写入路径在 Rust 侧被拒绝。
2. **反代写**：AI 输出仅限五类动作白名单；疑似连续正文直接丢弃。正文的每一个字都来自人。
3. **出站最小化**：仅 `EgressBundle` 可出站；无完整目录、无完整文件、无本机路径；发送前明示数量。
4. **无损 Markdown**：磁盘上的 `.md` 是唯一真相；编辑器与导出不得悄悄改写用户文件（换编辑器实现前必须先建 round-trip 语料测试）。
5. **派生可重建**：删索引库不丢用户资产——引用快照、思考卡、求助摘要、保存基线不是缓存。

---

## 五、数据与持久化

- **SQLite** 位于应用数据目录（`insightforge.sqlite`），由单一 Rust repository 独占所有权；renderer 不接触 SQL，不经通用 FS/SQL 插件。
- WAL 模式 + 单一写入队列；固定使用包含 2026 WAL-reset 修复的 SQLite（≥ 3.51.3）。
- FTS 表、扫描游标、缓存均为派生数据，可完全重建。

---

## 六、界面与命令边界

| 视图 | 职责 |
| --- | --- |
| 工作台 | 可调三栏（编辑器居中，正文宽度下限 35% + 证据架/检索侧栏）；陪练浮层仅在「我卡住了」后出现。 |
| 来源管理 | 目录授权、忽略规则、索引状态与进度。 |
| 设置 | AI（BYOK、模型、总开关）、同步间隔、忽略规则、语言。 |

- 前端只调用**窄而有类型的业务命令**（见第三节清单），无任意 FS/网络/SQL 权限。
- 领域规则（知识库只读、草稿目录可写、symlink 不越界、路径不上传）在 Rust 业务服务内执行，不靠通用 FS scope。

---

## 七、命名约定

- 用户可见文案**一律走 i18n**（`react-i18next`，`zh-CN`/`en-US`），禁止硬编码；纯函数返回 i18n key 而非文案。
- 交互元素带 `data-el="..."` 便于定位与测试。
- 编辑器实现藏在 `EditorAdapter` 适配层后。
- 导出格式化逻辑在 TypeScript 侧（remark 管线），不进 Rust、不塞编辑器。
- 阻塞型工作（扫描/解析/hash/写入）必须带显式 cancellation token。

---

## 八、旧版术语对照（Web ≤ 0.3.9 → 桌面版）

| 旧版术语 | 桌面版 | 说明 |
| --- | --- | --- |
| 碎片 `Fragment`（粘贴的笔记） | 片段 `Fragment`（索引分块） | **同名不同义**，重写时注意。 |
| 簇 `Cluster` | —（无） | 不再做自动归簇。 |
| 空白 `Gap` | —（无） | 选题发现由检索取证 + 陪练取代。 |
| 骨架 `Skeleton` | 陪练动作 `directions` | 提纲角色由陪练在卡住时接续。 |
| 护栏 `Guardrails`（字数占比/展开/流入） | 三轮协议上限 + 出站片段上限 | AI 根本不写进草稿，无需字数占比。 |
| 回流 `Reflow` | —（无，知识库只读） | 未来可把成文目录加为来源。 |
| 成文 `Writing` | 草稿 `.md` 文件本身 | 草稿即成文，无发布快照实体。 |
| 检索命中 `SearchHit`（fragment/writing） | 证据卡 `EvidenceCard` | 统一指向来源片段。 |
| MCP / API 路由 / Postgres / localStorage | 桌面 / SQLite | 全部移除。 |
| 环境变量（`OPENAI_*` 等） | 应用内设置 + OS 凭据库 | 无 `.env`。 |
