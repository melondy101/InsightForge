# InsightForge 绿色桌面架构选型

> 调研日期：2026-08-29  
> 范围：Windows 优先、未来跨平台、纯桌面、本地优先；不保留 Web 应用或服务端数据库。  
> 资料原则：仅使用项目官方文档、官方仓库、平台规范与 SQLite 官方文档。

## 结论

采用 **Tauri 2 + React/TypeScript + Vite + Rust 本地应用核心 + SQLite/FTS5**，按新应用从零设计，不迁移现有应用数据，也不保留 Next.js SSR、API routes、Postgres 或浏览器存储兼容层。

核心边界是：

- React 只负责工作台、Markdown 编辑器、证据卡和陪练交互；
- Rust 独占目录授权、文件读取与监听、增量索引、SQLite、草稿写入、冲突检测、AI 出站裁剪和系统凭据；
- 前端仅调用窄而有类型的命令，不获得任意文件系统、任意网络或任意 SQL 权限；
- 知识库目录严格只读，草稿/成文目录单独授权为可写；
- SQLite 位于应用数据目录，是可重建的派生索引和产品元数据，不污染知识库目录。

这不是因为当前项目已经使用 Tauri，而是因为它在本需求下取得了最好的组合：Web 编辑器生态、较轻的桌面封装、Rust 本地处理能力、明确的最小权限模型，以及现成的签名更新和跨平台打包链路。Tauri 使用系统 WebView；Windows 使用 WebView2，Windows 10/11 通常已随系统提供。需要离线捆绑 WebView2 时，官方列出的安装包增量约为 127 MB（offline installer）或 180 MB（fixed runtime），所以首发应使用系统 WebView + bootstrapper。[Tauri 架构](https://v2.tauri.app/concept/architecture/)，[Windows 安装方式](https://v2.tauri.app/distribute/windows-installer/)

## 需求适配

目标负载为最多约 10,000 个文件、2 GB 文本资料。应用运行时监听多个用户选择的本地目录，启动时补扫，也允许手动同步与间隔校验；App 关闭后不驻留。首版索引 `.md`、`.txt`，以后由转换适配器加入其他格式。知识库文件永不被应用修改。

草稿是真实、用户可见的 `.md` 文件。数据库只保存来源、片段、引用、布局、写作意图、求助摘要、思考卡等元数据。AI 是可关闭的 BYOK 增强层，只能收到当前求助所需的草稿窗口、已引用素材和少量命中片段，不得收到完整目录、完整文件或本机路径。

## 现实方案比较

| 维度 | Tauri 2 | Electron | Wails | Flutter |
|---|---|---|---|---|
| UI / 编辑器生态 | 直接使用 React 和 Web 编辑器；Windows 为 WebView2 | 直接使用 React；自带固定 Chromium，跨 OS 渲染最一致 | 同样可使用 React/Vite 和系统 WebView | 自有 Dart/Widget 生态；需要重写 Web UI，Markdown 富编辑器选择与互操作成本更高 |
| 本地核心 | Rust 很适合文件、并发、SQLite 和平台 API | Node 主进程/utility process；团队全 TS 时开发最快 | Go 后端清晰，生成 TS bindings | Dart FFI/插件可实现，但本需求会大量依赖桌面原生集成 |
| 运行时与分发 | 不捆绑浏览器；依赖系统 WebView | 随应用分发 Chromium + Node，包体和常驻资源基线更高 | 不捆绑浏览器；依赖系统 WebView | 自带 Flutter engine；无 WebView 差异，但运行时更重于系统 WebView 方案 |
| 文件与 SQLite | Rust 原生库；Tauri 也有官方 FS watch/SQL 插件 | Node API 丰富，但原生 SQLite 模块需处理 Electron ABI rebuild | Go 的 fsnotify/SQLite 生态可用 | `watcher`、`sqlite3` 等 Dart 包支持桌面，但关键插件需逐平台验证 |
| 安全边界 | Tauri 2 原生 permissions/scopes/capabilities | 能做到安全，但需自行严格实施 sandbox、contextIsolation、CSP 和 IPC sender 校验 | 暴露绑定方法的模型直观，细粒度能力系统不如 Tauri 完整 | 插件/平台通道由应用自行设计权限边界 |
| 更新 / 测试 | 官方 updater 强制签名；WebDriver 与 renderer-only 测试 | Forge、autoUpdater 和成熟 Chromium 调试；Playwright Electron API仍标为 experimental | v2 稳定但能力较旧；v3 有 updater 等改进但截至调研日仍为 beta | 官方桌面 integration test；Windows 打包文档偏 Microsoft Store/MSIX |
| 主要代价 | Rust 学习与编译成本；必须测试多种系统 WebView | 更重；native module ABI 和 preload/IPC 安全维护 | v2/v3 代际选择不理想；v3 尚未 GA | 放弃 React 编辑器生态并整体转向 Dart，收益不足以抵消重写成本 |

### 为什么不是 Electron

Electron 把 Chromium 和 Node.js 嵌入并随应用分发，这是它实现稳定统一渲染的来源，也是更高安装体积与运行时基线的来源。[Electron 官方定义](https://www.electronjs.org/docs/latest/)，[应用分发](https://www.electronjs.org/docs/latest/tutorial/application-distribution)

它的 `utilityProcess` 很适合隔离索引工作，统一 Chromium 也能减少 macOS/Linux 的编辑器渲染差异。[utilityProcess](https://www.electronjs.org/docs/latest/api/utility-process) 但本产品的核心复杂度恰好在长期运行的文件索引与敏感目录访问：Electron 官方安全清单要求应用自行持续落实 context isolation、sandbox、CSP、导航限制和 IPC 发送方验证；原生 Node 模块又可能因 Electron ABI 与普通 Node 不同而需要 rebuild。[Electron 安全清单](https://www.electronjs.org/docs/latest/tutorial/security)，[原生 Node 模块](https://www.electronjs.org/docs/latest/tutorial/using-native-node-modules)

若团队明确拒绝 Rust、希望所有代码都留在 TypeScript，并愿意接受更大的包体与更严格的 IPC 工程纪律，Electron 是合理的第二选择；否则不是本项目的首选。

### 为什么不是 Wails

Wails 同样复用系统 WebView，支持 React/Vite，Go 方法可生成 TypeScript bindings，Windows 使用 WebView2，能够做出轻量且开发体验不错的应用。[Wails 介绍](https://wails.io/docs/introduction/)，[工作原理](https://wails.io/docs/howdoesitwork/)

问题在版本时点：Wails v2 是当前稳定版，但应用模型相对旧；改进了多窗口、服务、打包和内置 updater 的 v3 截至 2026-08-29 仍为 beta，官方也明确要求在部署前充分测试。[Wails v3 状态](https://v3.wails.io/status/)，[Wails v3 FAQ](https://v3.wails.io/faq/) 对一个要长期维护的绿色应用，选 v2 意味着较早面对一次真实迁移，选 v3 则承担预发布框架风险。Go 本身很适合索引服务，但不足以抵消这一时点风险。

### 为什么不是 Flutter / 原生 UI

Flutter 正式支持 Windows、macOS 和 Linux，可创建平台插件并运行桌面 integration tests；Windows 也有 MSIX/Store 分发文档。[Flutter 桌面支持](https://docs.flutter.dev/platform-integration/desktop)，[桌面集成测试](https://docs.flutter.dev/testing/integration-tests)，[Windows 分发](https://docs.flutter.dev/deployment/windows)

它适合希望完全控制渲染、共享移动端 UI 的产品。本项目没有移动端目标，却强依赖成熟的 Web Markdown 编辑器和 React 交互生态。转向 Flutter 会把 UI、编辑器扩展、无损 Markdown 互操作和桌面插件集成同时变成新工作，未换来与需求同等重要的收益。纯 WinUI/Swift/Qt 也有类似问题，并会削弱未来跨平台的一套 UI。

## 推荐的系统形状

```text
React / TypeScript / Vite
  ├─ Workspace UI（可调三栏，正文宽度下限 35%）
  ├─ Markdown editor adapter
  ├─ Evidence shelf
  └─ Coach UI（仅显式“我卡住了”后出现）
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

Tauri 能运行 blocking 工作于专门 executor，适合把扫描、解析、hash 与数据库写入移出 UI 路径；任务必须有显式 cancellation token，因为已经开始的 blocking work 不能只靠丢弃 handle 停止。[Tauri `spawn_blocking`](https://docs.rs/tauri/latest/tauri/async_runtime/fn.spawn_blocking.html)

### 前端与编辑器

使用 React + TypeScript + Vite，不使用 Next.js。纯桌面静态前端不需要 SSR、服务器组件或 API routes，Vite 的模型更小、更直接。

首版建议用 **CodeMirror 6 的 Markdown source editor**，通过装饰、侧栏卡片和预览提供“富体验”，而不是让富文本 AST 成为文件真相。CodeMirror 把文档保持为平面字符串，状态/视图/扩展分离，适合无损保存用户的 Markdown，同时支持大文档、键盘、屏幕阅读器和自定义 widget。[CodeMirror 系统指南](https://codemirror.net/docs/guide/)，[CodeMirror 功能](https://codemirror.net/)

把编辑器置于 `EditorAdapter` 后；如果真实用户测试表明源码式 Markdown 门槛过高，可替换为 ProseMirror/Lexical 类编辑器，但必须先建立 Markdown round-trip 语料测试，避免悄悄改写用户文件。

### 本地索引

不要让 watcher 成为事实来源。文件事件只负责尽快触发工作，启动补扫、手动同步和 5/15/30 分钟校验才负责最终一致性。跨平台 watcher 官方说明对网络文件系统和超大目录可能遗漏事件；因此这一设计是必要的。[Rust `notify` 文档](https://docs.rs/notify/latest/notify/)

增量流程：

1. 规范化并验证被授权根目录；默认不跟随逃出根目录的 symlink。
2. 遍历允许的扩展名与 ignore globs，记录稳定文件标识（可用时）、规范路径、size、mtime 和内容 hash。
3. 仅对新增或 hash 改变的文件解析；Markdown 优先按标题，其次按段落与长度窗口分块。
4. 在一个 SQLite transaction 中替换该文件的片段与 FTS 行；事件通过 channel 推送进度。
5. 删除文件退出未来检索，但已被文章引用的片段保留不可变快照并标记来源不可用。
6. 重命名优先用 file identity，其次用内容 hash + 时间窗口识别，避免删除再新增。

SQLite 使用单一 Rust repository 所有权，不通过 Tauri SQL plugin 向 renderer 暴露 SQL。启用 FTS5；它内置 `bm25()`、`highlight()` 和 `snippet()`，足以完成首版关键词排序和证据片段生成。[SQLite FTS5](https://www.sqlite.org/fts5.html) 中文首版应实测 `trigram` tokenizer；SQLite 官方说明它把连续三个字符作为 token 并支持一般的 substring matching。两字符查询需回退到受限线性扫描或另一索引。[FTS5 trigram](https://www.sqlite.org/fts5.html#the_trigram_tokenizer)

索引数据库只在本机应用数据目录。WAL 能让读写并行，但仅允许单一写入队列，并固定使用包含 2026 WAL-reset 修复的 SQLite 版本；官方说明该问题已在 3.51.3 及列出的 backport 中修复。[SQLite WAL](https://www.sqlite.org/wal.html)

### 草稿保存与冲突

打开草稿时保存 `{file identity, content hash, mtime, size}` 基线。每次保存先重新读取元数据/hash；若外部版本已变化，返回结构化 `Conflict`，让用户查看 diff 并选择保留本地、采用外部或另存为，绝不静默覆盖。

正常保存采用同目录临时文件：写入、flush、sync，然后通过平台适配器替换目标。Windows 使用 `ReplaceFileW`；该 API 把“写新文件、交换名称、删除旧文件”合并为一个平台操作，并可保留原文件属性。[Microsoft `ReplaceFile`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew) Unix 使用同文件系统 rename，并为 Windows/macOS/Linux 建立断电、进程崩溃、锁占用和杀毒软件干扰的集成测试。只有替换成功后才更新数据库基线。

### 权限与 AI 数据边界

只注册业务命令，例如：

- `sources.add/list/pause/remove/sync`
- `search.query/open_context`
- `drafts.open/save/resolve_conflict`
- `coach.start/respond/resume/mark_helpful`
- `thoughts.confirm/update`

Tauri 2 的 permissions、scopes 和 capabilities 可以把命令与窗口绑定；FS 插件默认阻止危险命令，并要求命令权限之外另有路径 scope。[Tauri permissions](https://v2.tauri.app/security/permissions/)，[Tauri FS 安全与 watch](https://v2.tauri.app/plugin/file-system/) 本项目仍应把文件访问放进 Rust 业务服务，因为动态目录授权还需要“知识库只读、草稿目录可写、symlink 不越界、路径不上传”等领域规则，不能只靠通用 FS scope。

每次远端 AI 调用先由 Rust 生成不可变 `EgressBundle`：当前选区或光标前后段落、标题/大纲、已引用素材、最多三条检索片段、无本机路径的来源标题。UI 显示“将发送 N 个片段”；只有该 bundle 可进入 HTTP client。响应必须解析为白名单动作 `question | evidence | tension | directions | micro_task`，疑似连续正文直接丢弃并替换成本地安全提示。

BYOK 存入系统凭据库适配层：Windows Credential Manager，未来 macOS Keychain、Linux Secret Service。Rust `keyring` 的一方文档列出了这三类平台 store。[Rust keyring](https://docs.rs/keyring/latest/keyring/v1/) Tauri Stronghold 是加密 vault，但不等同操作系统凭据库，不应混为一谈。[Tauri Stronghold](https://v2.tauri.app/plugin/stronghold/)

## 建议的数据骨架

不做旧数据迁移；首次运行建立全新 schema。最小表如下：

| 表 | 用途 |
|---|---|
| `knowledge_source` | 用户命名的根目录、启停、ignore、同步游标 |
| `source_file` | 相对路径、file identity、hash、mtime、状态；完整路径不进入 AI 数据 |
| `fragment` | 文件内定位、标题路径、文本、内容 hash |
| `fragment_fts_*` | FTS5 派生索引；可完全重建 |
| `draft` | 外部 Markdown 路径标识、保存基线、写作意图、布局 |
| `draft_reference` | 草稿位置、片段 ID、不可变引用快照、采用状态 |
| `coach_session` | 三轮状态、接续条件、摘要、帮助反馈 |
| `coach_turn` | 本机会话轮次与结构化动作；发布后默认折叠且不进入检索 |
| `thought_card` | 作者编辑确认的想法、素材与草稿位置关系 |
| `ai_call_audit` | 模型、时间、发送片段数量/ID、状态；不重复保存敏感全文 |

FTS 表、扫描游标和普通缓存均为派生数据；引用快照、思考卡、求助摘要和草稿保存基线不是缓存，不能随“重建索引”删除。

## 打包、更新与测试

Windows 首发产出签名的 NSIS 或 MSI installer。Tauri updater 强制校验签名且不可关闭，可用静态 JSON/GitHub Releases，不需要自建业务服务器。[Tauri updater](https://v2.tauri.app/plugin/updater/) 首发不捆绑 fixed WebView2；为确有离线安装需求的组织另产 offline installer。

测试分三层：

1. Rust 单元/属性测试：分块、路径边界、ignore、hash 迁移、排名、三轮状态机、AI 出站裁剪。
2. 临时目录 + SQLite 集成测试：新增/修改/删除/重命名、watch 漏事件后的 reconciliation、10k 文件基准、草稿冲突与失败恢复。
3. Windows 桌面 E2E：目录选择、索引进度、证据卡、编辑保存、外部冲突、AI consent。Tauri 官方支持 WebdriverIO Tauri service，也提供纯浏览器 renderer 模式；后者不能代替真实文件与安装测试。[Tauri WebDriver](https://v2.tauri.app/develop/tests/webdriver/)

macOS/Linux 尚未正式发布时，CI 至少编译 Rust core 并跑平台无关测试；正式宣称支持前，必须在真实 WKWebView/WebKitGTK 上跑编辑器输入法、selection、clipboard、drag/drop 与 E2E。系统 WebView 的差异是 Tauri 相对 Electron 最重要的长期代价。

## 显式取舍与决策门槛

接受的代价：

- 用 Rust 换取更可靠的本地系统核心与更窄的安全边界；
- 用系统 WebView 换取更轻的分发，承担跨 OS 编辑器兼容测试；
- 首版只做本地关键词检索，不引入 embedding、向量数据库或后台守护进程；
- 用 Markdown source-of-truth 换取无损互操作，不承诺第一版就有 Word 式 WYSIWYG；
- 删除旧应用数据且不维护迁移路径，换取清晰的新 schema 和更少历史包袱。

在正式全面实现前做一个两周以内的架构曳光弹，只验证五件高风险事项：

1. Windows 中文输入法下 CodeMirror 长文编辑与光标窗口提取；
2. 10k 文件的首次索引、增量更新和 trigram 中文搜索质量/索引体积；
3. 外部编辑导致的冲突检测与 `ReplaceFileW` 保存恢复；
4. 动态授权目录的只读边界和 symlink/path traversal 测试；
5. 签名 NSIS/MSI + updater 的安装/升级闭环。

若这五项通过，Tauri 2 方案继续；只有在 WebView2 编辑器兼容性出现不可接受且无法隔离的问题时，才应回退 Electron。不要因为 Rust 初期开发速度较慢就过早换架构，那是可预期的学习成本；真正的否决条件应是核心体验或平台可靠性不达标。
