# 织知 InsightForge — 桌面版路线图

> 版本：1.0（桌面版重写稿）
> 最后更新：2026-09-02
> 依据：[docs/research/2026-08-29-desktop-architecture-options.md](./research/2026-08-29-desktop-architecture-options.md) · [docs/research/2026-08-30-platform-export-options.md](./research/2026-08-30-platform-export-options.md)

---

## 0. 定调

- 旧 Web 版止于 **0.3.9**，不再演进；桌面版从 **0.1.0** 重新计数，**不迁移旧数据**。
- 技术底座已定：**Tauri 2 + React/TS/Vite + Rust 核心 + SQLite/FTS5**，理由与对比见 ARCHITECTURE.md 与调研存档。
- 方法论不变：TDD + 垂直切片（曳光弹），每轮「一个测试 → 最小实现 → 变绿」，任何时候不超过一个失败测试（见 AGENTS.md）。

### 实施状态（2026-09-02）

- M1–M3 的实现以及双导出能力已合入 `main`，索引-搜索通过 `sources_await_index_committed` 建立可观察的提交时序契约。
- Windows 个人自用可继续验证；M4 的公开发布门槛仍未关闭：签名安装包、HTTPS updater 与安装→升级 E2E 均待真实发布条件。

---

## 1. M0 · 架构曳光弹（≤ 2 周，内部验证，不发布）

在正式全面实现前，只验证五件高风险事项：

| # | 验证内容 | 通过标准 |
| --- | --- | --- |
| 1 | Windows 中文输入法下 CodeMirror 长文编辑 | 连续输入无丢字/错位；能提取光标前后段落供 EgressBundle 使用 |
| 2 | 10k 文件首次索引、增量更新、trigram 中文搜索 | 索引时长与体积可接受；bm25+trigram 召回质量达标；两字符回退可用 |
| 3 | 外部编辑冲突检测与原子保存 | 外部修改触发结构化 `Conflict`；`ReplaceFileW` 保存 + 断电/崩溃恢复用例通过 |
| 4 | 动态授权目录的只读边界 | symlink / path traversal 逃逸用例全部拒绝；源文件内容 hash 前后不变 |
| 5 | 签名 NSIS/MSI + updater | 安装 → 升级全链路闭环，签名校验不可绕过 |

**闸门**：五项全过 → Tauri 2 方案继续。仅当 WebView2 编辑器兼容性出现不可接受且无法隔离的问题时，才回退评估 Electron。Rust 初期开发速度较慢是可预期学习成本，不构成换架构的理由。

---

## 2. M1 · 来源与本地检索（0.1.0，内部可用）

**范围**：`sources.add/list/pause/remove/sync`；遍历 / ignore / hash；增量事务索引；FTS5 检索 UI；索引进度事件；SQLite 落库与迁移。

**完成标准（可观察行为）**：

- 添加目录后启动补扫完成、进度可见；新增/修改/删除/重命名后检索结果正确更新（含 watch 漏事件后由补扫收敛）。
- 删除的文件退出检索；被引用片段保留快照并标记「来源不可用」。
- 中文关键词检索返回 bm25 排序与 snippet 上下文。
- 只读边界：路径逃逸 / symlink 越界用例全部通过；源文件内容 hash 前后不变。

---

## 3. M2 · 写作工作台（0.2.0）

**范围**：可调三栏工作台（正文宽度下限 35%）；CodeMirror 6 编辑器（`EditorAdapter` 后）；`drafts.open/save/resolve_conflict`；证据架与不可变引用快照。

**完成标准**：

- 打开/编辑/保存输出目录 `.md`；保存为原子替换，断电/崩溃/锁占用恢复用例通过；仅替换成功后才更新基线。
- 外部修改后保存返回 `Conflict`，diff 三选一（保留本地/采用外部/另存为），无静默覆盖。
- 中文 IME 长文输入无损；Markdown round-trip 语料测试全绿（文件不被悄悄改写）。
- 证据卡引用进草稿生成不可变快照；来源删除后快照保留并标记「来源不可用」。

---

## 4. M3 · 陪练与 AI 边界（0.3.0）

**范围**：BYOK 入系统凭据库；`ai_gateway`（EgressBundle + OpenAI 兼容 client + 审计）；`coach.start/respond/resume/mark_helpful` 三轮协议；`thoughts.confirm/update` 思考卡；AI 总开关。

**完成标准**：

- 无 Key / 未开启 AI 时，除 AI 外全功能可用（AI 缺席即降级，无需兜底文案伪装）。
- 每次 AI 调用前 UI 明示「将发送 N 个片段」，实际载荷恰为 `EgressBundle`（无本机路径、≤3 条片段）；审计落库。
- 响应白名单校验：注入式连续正文被丢弃并替换为本地安全提示。
- 三轮结束会话终止、把笔还给作者；发布后陪练轮次折叠且不进入检索。

---

## 5. M4 · 导出与分发（1.0.0，首个对外发布）

**范围**：`generic-html` 与 `wechat-clipboard-html` 双导出目标；`Clean Reading` 固定主题 + 强调色；签名 NSIS/MSI 安装包 + updater。

**完成标准**：

- 通用 HTML：语义化、已消毒、含阅读排版样式；公众号 HTML：标签/属性白名单、样式内联，双写 `text/html` 与 `text/plain` 剪贴板。
- 不可访问的远程图片阻断导出并明确提示，不产生表面成功实际丢图的内容。
- 签名安装 → updater 升级闭环；E2E：目录选择 → 索引 → 检索 → 写作 → 保存 → 导出全链路通过。

---

## 6. 明确不做（当前路线图内）

微信 API 直发 / 草稿箱同步；embedding / 向量检索；后台驻留 / 多用户 / 云同步；Word / PDF 导出；自定义主题 / 插件市场；知识库写回；旧数据迁移。

---

## 7. 未来探索（需重新立项评估，不进入当前承诺）

- **macOS / Linux 支持**：正式宣称前，必须在真实 WKWebView / WebKitGTK 上通过编辑器输入法、selection、clipboard、drag/drop 与 E2E。
- **离线 WebView2 安装包**：面向确有离线安装需求的组织（+127~180 MB）。
- **更多输入格式**：通过转换适配器加入 `.docx` / `.html` 等（首版只有 `.md` / `.txt`）。
- **成文目录作为知识来源**：把草稿/成文目录加为只读来源，替代旧版「回流飞轮」。
- **WordPress 传输适配器**：在通用 HTML 之上增加「推送草稿」能力（Posts REST API）。
- **创作者身份档案回归**：影响陪练取向（只影响方向，仍不代写）。
- **编辑器替换评估**：若真实用户测试表明源码式 Markdown 门槛过高，在 round-trip 语料测试保护下替换为 ProseMirror / Lexical 类编辑器。

---

## 8. 设计原则（不变）

1. **反代写**：AI 只递问题与证据，绝不成段代写。
2. **本地优先**：知识库只在用户自己的文件夹里，索引可重建，除 AI 出站外无网络行为。
3. **最小权限**：前端只拿窄而有类型的命令；文件与密钥边界在 Rust 侧强制执行。
4. **无损 Markdown**：磁盘文件是唯一真相。
5. **用户掌控**：创作者是唯一表达主体，AI 是陪练，不是代笔。
