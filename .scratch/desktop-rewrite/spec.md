# Spec · 织知 InsightForge 桌面版 1.0.0（绿色重写）

> Status: ready-for-agent
> 日期：2026-08-30
> 范围：桌面版全量（ROADMAP M1–M4 → 1.0.0）
> 依据：docs/PRD.md · docs/ARCHITECTURE.md · docs/ROADMAP.md · CONTEXT.md · docs/research/（调研存档）

## Problem Statement

创作者的知识散落在自己电脑的 `.md` / `.txt` 文件里。市面上的 AI 写作工具要求把笔记上传到云端、并急着替你把文章写完——前者泄露隐私，后者稀释表达。旧版织知（Web ≤ 0.3.9）验证了「反代写」理念，但要求手动粘贴碎片、依赖浏览器存储与云数据库，无法直接索引已有的本地资料库，也没有对 AI 出站内容的硬边界。

## Solution

一个 Windows 优先的纯桌面应用（Tauri 2 + Rust 本地核心）：

- 用户把自己的文件夹**授权为只读知识库来源**，应用本地索引、FTS5 中文关键词检索出**证据卡**；
- 在**真实 `.md` 文件**上写作，保存原子化，外部修改以 diff 呈现、绝不静默覆盖；
- 卡住时显式召唤**陪练**：AI 只输出提问 / 证据 / 张力 / 方向 / 微任务五类动作，三轮即止，把笔还给作者；
- 每次 AI 出站内容事先明示「将发送 N 个片段」且最小化，BYOK 密钥入系统凭据库，调用留审计；
- 写完一键导出为 WordPress 可用的完整 HTML，或公众号剪贴板格式。

知识库文件永不被修改，索引库可全量重建，AI 可整体关闭，关闭应用即退出不驻留。

## User Stories

**来源与索引**

1. As a 创作者, I want 把我的笔记文件夹添加为知识库来源并命名, so that 应用能检索我已有的知识而无需上传或搬移它们
2. As a 创作者, I want 随时暂停或恢复某个来源的同步, so that 我能控制哪些目录参与检索
3. As a 创作者, I want 为来源配置忽略规则, so that 临时文件、构建产物等噪声不进入索引
4. As a 创作者, I want 移除来源时只清理索引记录、绝不动源文件, so that 我能放心试错
5. As a 创作者, I want 添加来源后看到索引进度, so that 我知道应用在做什么、何时可用
6. As a 创作者, I want 新增、修改、删除、重命名文件后检索结果自动更新, so that 检索永远反映磁盘现状
7. As a 创作者, I want 应用重启后通过补扫收敛索引, so that 即使错过文件事件也不会留下过期索引
8. As a 创作者, I want 手动触发同步, so that 不依赖文件监听也能保持索引准确
9. As a 隐私敏感用户, I want 知识库文件绝不被应用修改, so that 我的原始资料完好无损
10. As a 创作者, I want 索引与检索不卡界面, so that 大目录首次索引时我仍能正常使用应用

**检索与证据**

11. As a 创作者, I want 用中文关键词检索知识库并按相关度排序, so that 卡住时能快速找到相关素材
12. As a 创作者, I want 检索结果带片段上下文和来源标题, so that 我能判断这条证据是否值得引用
13. As a 创作者, I want 两字词也能搜到结果, so that 短查询不空手而归
14. As a 创作者, I want 证据卡可以固定在证据架上, so that 写作时素材触手可及
15. As a 创作者, I want 从证据卡跳到来源文件上下文, so that 我能核实原文再引用

**草稿写作**

16. As a 创作者, I want 在真实 `.md` 文件上写作, so that 我的文章不被专有格式锁定、任何工具都能打开
17. As a 创作者, I want 保存是原子操作, so that 断电或崩溃不会损坏草稿
18. As a 创作者, I want 外部编辑器改动草稿后，保存时看到 diff 并三选一（保留本地/采用外部/另存为）, so that 任何一边的修改都不会被静默覆盖
19. As a 创作者, I want 调整三栏布局且正文宽度有下限, so that 长时间写作舒适
20. As a 中文创作者, I want 输入法在长文中流畅无损, so that 编辑器不拖累表达
21. As a 创作者, I want 引用素材时草稿里留下不可变引用快照, so that 即使来源文件后来被删，成文引用依然完整
22. As a 创作者, I want 来源不可用的引用被明确标记, so that 我知道哪些引用需要核实
23. As a 创作者, I want Markdown 原稿不被编辑器悄悄改写, so that 文件永远是我的唯一真相

**陪练（Coach）**

24. As a 卡住的创作者, I want 显式说「我卡住了」后获得提问/证据/张力/方向/微任务, so that 我能重新动笔而不是等 AI 代笔
25. As a 反代写理念认同者, I want AI 的任何连续正文输出被丢弃并给出本地安全提示, so that 工具永远不会替我写
26. As a 创作者, I want 陪练最多三轮后结束, so that 会话把笔还给我而不是无限纠缠
27. As a 创作者, I want 对陪练轮次标记有帮助与否, so that 我能自己评估陪练质量
28. As a 创作者, I want 草稿发布后陪练轮次默认折叠且不进入检索, so that 会话噪声不污染知识检索

**AI 边界与配置**

29. As a 隐私敏感用户, I want 每次 AI 调用前看到「将发送 N 个片段」, so that 我对每次出站内容有知情权
30. As a 隐私敏感用户, I want 完整目录、完整文件、本机路径永不出站, so that 我的文件系统布局不被泄露
31. As a 用户, I want 不配 Key 或关闭 AI 时除 AI 外全功能可用, so that AI 是增强而非依赖
32. As a 用户, I want BYOK 密钥存入系统凭据库, so that 密钥不以明文形式落在数据库或前端
33. As a 用户, I want 查看 AI 调用审计（模型、时间、发送片段数、状态）, so that 出站行为可追溯

**思考卡**

34. As a 创作者, I want 确认「想法 ↔ 素材 ↔ 草稿位置」的关系为思考卡, so that 写作中的灵感不被丢失
35. As a 创作者, I want 思考卡在重建索引后依然存在, so that 我的思考资产不随派生缓存消失

**导出发布**

36. As a 创作者, I want 把草稿导出为语义化、已消毒、带阅读排版样式的完整 HTML, so that 能贴进 WordPress 等任何 CMS
37. As a 创作者, I want 一键复制公众号格式（样式内联）的 HTML 并粘贴进微信编辑器, so that 不用手动调格式
38. As a 创作者, I want 图片不可访问时导出被阻断并明确提示, so that 不会发布表面成功实际丢图的文章
39. As a 创作者, I want 导出过程不反向修改 Markdown 原稿, so that 导出是无损的派生物

**分发与体验**

40. As a Windows 用户, I want 下载签名安装包并通过应用内更新获得新版本, so that 安装与升级安全省心
41. As a 用户, I want 关闭应用后没有任何后台进程驻留, so that 我的机器不被占用
42. As a 用户, I want 中文/英文界面切换, so that 我用自己的语言工作

## Implementation Decisions

**架构底座**

- Tauri 2 + React/TypeScript/Vite 前端 + Rust 应用核心 + SQLite/FTS5，按新应用从零设计；**不迁移旧 Web 版数据**，不保留 Next.js/SSR/API routes/Postgres/localStorage/MCP。
- React 只负责工作台、编辑器、证据卡与陪练交互；Rust 独占目录授权、文件读取与监听、增量索引、SQLite、草稿写入、冲突检测、AI 出站裁剪和系统凭据。
- AI 接入继续遵循 ADR 0001 的 BYOK 决策（OpenAI 兼容 API），密钥存 OS 凭据库（Windows Credential Manager；macOS Keychain / Linux Secret Service 为后续）。

**公共接口（唯一 seam 的形状）**

- Rust 暴露的业务命令清单（决定编码自调研文档，不再是想象接口）：
  `sources.add / list / pause / remove / sync`；`search.query / open_context`；`drafts.open / save / resolve_conflict`；`coach.start / respond / resume / mark_helpful`；`thoughts.confirm / update`。
- 索引进度等状态经事件通道推送；阻塞型工作（扫描/解析/hash/写入）运行在专门 executor 且必须带显式 cancellation token。
- Tauri permissions/scopes/capabilities 只做命令与窗口绑定；动态目录授权的领域规则（知识库只读、草稿目录可写、symlink 不越界、路径不上传）在 Rust 业务服务内强制执行，不靠通用 FS scope。

**索引与检索**

- watcher 不是事实来源：启动补扫、手动同步、5/15/30 分钟间隔校验负责最终一致；文件事件只负责尽快触发。
- 增量算法：验证授权根目录 → 遍历扩展名与 ignore globs，记录稳定文件标识、规范路径、size、mtime、内容 hash → 仅解析新增或 hash 改变的文件 → Markdown 优先按标题、其次按段落与长度窗口分块 → 单个 SQLite 事务内替换该文件的片段与 FTS 行。
- 删除文件退出检索，被引用片段保留不可变快照并标记「来源不可用」；重命名优先按 file identity，其次按内容 hash + 时间窗口；默认不跟随逃出根目录的 symlink；首版输入格式 `.md` / `.txt`。
- FTS5 `bm25()` 排序 + `highlight()`/`snippet()` 上下文；中文实测 trigram tokenizer，两字符查询回退受限线性扫描。

**数据（Schema 决策，来自调研文档）**

- SQLite 位于应用数据目录，由单一 Rust repository 独占；WAL + 单一写入队列；固定使用包含 2026 WAL-reset 修复的 SQLite（≥ 3.51.3）。
- 十张表：`knowledge_source`（根目录、启停、ignore、同步游标）、`source_file`（相对路径、file identity、hash、mtime、状态；完整路径不进入 AI 数据）、`fragment`（文件内定位、标题路径、文本、内容 hash）、`fragment_fts_*`（派生索引，可重建）、`draft`（路径标识、保存基线、写作意图、布局）、`draft_reference`（草稿位置、片段 ID、不可变引用快照、采用状态）、`coach_session`（三轮状态、接续条件、摘要、帮助反馈）、`coach_turn`（轮次与结构化动作；发布后折叠且不进检索）、`thought_card`（想法/素材/草稿位置关系）、`ai_call_audit`（模型、时间、片段数量/ID、状态）。
- 派生数据（FTS 表、扫描游标、缓存）可全量重建；引用快照、思考卡、求助摘要、保存基线不是缓存，不随重建删除。

**草稿与编辑器**

- 草稿是授权输出目录中的真实 `.md` 文件；打开时记录保存基线（file identity、content hash、mtime、size），保存前重读比对，外部已变化则返回结构化 `Conflict`，diff 三选一，绝不静默覆盖。
- 原子保存：同目录临时文件写入 → flush → sync → 平台替换（Windows `ReplaceFileW`，Unix 同文件系统 rename）→ 成功后才更新基线。
- 编辑器首版为 CodeMirror 6 Markdown source editor，通过装饰/侧栏卡片/预览提供富体验；藏在 `EditorAdapter` 适配层后，替换实现前必须先建立 Markdown round-trip 语料测试；三栏布局正文宽度下限 35%。

**陪练与 AI 边界**

- 三轮协议：仅显式「我卡住了」触发；响应必须解析为白名单动作（类型形状来自调研文档）：`question | evidence | tension | directions | micro_task`；疑似连续正文直接丢弃并替换本地安全提示。
- 每次出站前由 Rust 生成不可变 `EgressBundle`：选区/光标前后段落、标题/大纲、已引用素材、最多三条检索片段、无本机路径的来源标题；UI 明示「将发送 N 个片段」，只有该 bundle 可进入 HTTP client。
- `ai_call_audit` 只记元数据不存全文；AI 总开关关闭后除 AI 外全功能可用。

**导出**

- 导出层在 TypeScript 侧：`remark-parse → remark-gfm → remark-rehype → rehype-stringify`（输出 HTML 必须加 sanitize），目标类型（来自调研文档）：`generic-html`（语义化、已消毒、含 `Clean Reading` 阅读排版的完整 HTML 文件）与 `wechat-clipboard-html`（标签/属性白名单、样式内联片段，双写 `text/html` 与 `text/plain` 剪贴板）。
- 一个固定主题 + 一个强调色；Markdown 内嵌 HTML 默认禁用；图片仅允许远程 `https` URL，不可访问时阻断导出并明确提示；不安全协议链接降级为纯文本。

**分发**

- Windows 首发签名 NSIS/MSI 安装包（系统 WebView2 + bootstrapper）；Tauri updater 强制签名校验，走静态 JSON/GitHub Releases，不自建业务服务器。

**产品约定**

- 用户可见文案一律走 `react-i18next`（zh-CN / en-US，默认中文），不硬编码。
- 交互元素带 `data-el` 定位属性；领域术语遵循 CONTEXT.md（注意：`Fragment` 在桌面版中指索引分块，非旧版粘贴笔记）。

## Testing Decisions

- **好测试的标准**：只断言通过公共接口可观察的行为（命令的输入输出、文件系统与数据库的最终状态），不断言内部实现；一次彻底的内部重写（例如换分块策略、换分词器实现）测试应依然全绿；每个测试只断言一个行为（AGENTS.md 清单）。
- **单一 seam**：Tauri 命令边界。在 Rust 集成测试中对**真实临时目录 + 真实 SQLite 文件** headless 调用命令函数，断言：只读边界与 symlink 拒绝、增删改重命名后的检索收敛（含 watch 漏事件后由补扫收敛）、bm25+trigram 排序与两字符回退、原子保存与断电/崩溃/锁占用恢复、冲突三选一、EgressBundle 载荷恰为承诺内容（≤3 条片段、无本机路径）、白名单校验丢弃连续正文、三轮终止条件、重建索引后快照/思考卡/基线存活。
- **辅助层**：Rust 单元/属性测试覆盖纯逻辑（分块、路径规范化、ignore 匹配、排名、三轮状态机、出站裁剪）；TypeScript 侧 vitest 覆盖导出管线与 Markdown round-trip 语料测试（这是新栈中第一组 TS 测试，将成为后续 TS 侧 prior art）。
- **UI 层**：前端是命令输出的薄投影，其 WebDriver E2E 按 ROADMAP 推迟到 M4 全链路验收（目录选择 → 索引 → 检索 → 写作 → 保存 → 导出）；本 spec 阶段不为 UI 写测试。
- **Prior art**：Rust 侧无既有测试（全新核心）；纪律与模式来自 AGENTS.md（TDD 垂直切片、任意时刻不超过一个失败测试、提交前三绿）。`coach` 的 LLM 依赖与 `ai_gateway` 的 HTTP 依赖在命令 seam 后注入假实现，不真发网络请求。
- **CI**：macOS/Linux 未支持期间，至少编译 Rust core 并跑平台无关测试。

## Out of Scope

- Web 应用、服务端 API、Postgres、MCP 服务；旧数据迁移。
- embedding / 向量检索 / 语义搜索；后台守护进程；多用户；云同步。
- 微信 API 直发、草稿箱同步、图片托管；base64 图片内嵌。
- 自定义 CSS、主题市场、复杂模板；Word 式 WYSIWYG 承诺。
- 知识库写回；`.md`/`.txt` 之外的输入格式。
- macOS / Linux 支持（须真实 WKWebView / WebKitGTK E2E 过关，见 ROADMAP 未来探索）；离线 WebView2 安装包。
- Word / PDF 导出。
- UI WebDriver E2E（M4 才纳入验收）。

## Further Notes

- **与 M0 曳光弹的关系**：ROADMAP 的 M0 五项验证不另立 spec——第 2、4 项（10k 索引/trigram、只读边界）就是本 spec 头几个垂直切片；第 1、3、5 项（IME、原子保存、签名安装）在对应里程碑开工前以 spike 形式先行验证。闸门不变：仅当 WebView2 编辑器兼容性问题不可接受且无法隔离时回退评估 Electron。
- **版本计数**：桌面版从 0.1.0 重新计数；旧 Web 版止于 0.3.9 冻结。PRD/CONTEXT/ROADMAP/ARCHITECTURE 为唯一事实来源，docs/research/ 为调研存档不再更新。
- **实施节奏**：按 ROADMAP M1→M4 垂直切片交付，一个功能一条 commit；可用 `/to-tickets` 把本 spec 拆为 `.scratch/desktop-rewrite/issues/` 下的实施票据。
- Issue tracker 尚未固化配置：建议运行 `/setup-matt-pocock-skills` 将本地 markdown 约定写入 `docs/agents/`，后续技能（triage、to-tickets、wayfinder）将直接读取。
