# 主控执行 Prompt · 桌面版重写

> 生成于 2026-08-30（to-spec → to-tickets 流程之后）。用途：粘贴给 Claude Code 主对话，按 `.scratch/desktop-rewrite/issues/01–15` 逐票执行，Builder / Reviewer / Finalizer 三重会话门禁 + worktree 隔离。执行前注意：当时工作树有未提交内容，Prompt 第 1 节要求先做基线提交。

## 一、前置分析存档（生成时结论）

**依赖图**

```
            ┌─> 02 ─> 03 ─> 04 ─┬─> 07 ─┬─> 09 ─> 10
            │                   │       └─> 11
01 ────────┼─> 05 ─> 06 ──────┼─> 12 ─> 13 ─┬─> 14
            │                   │             └─> 15（实际闸门 = 10 + 13 + 14）
            └─> 08 ────────────┘
（06 → 09；07 → 09、11；04 → 07、11）
```

**并行波次**：W0 基线提交 → W1: 01 → W2: 02∥05∥08 → W3: 03∥06 → W4: 04∥12 → W5: 07∥13 → W6: 09∥11 → W7: 10 → W8: 14 → 15。不应并行：02→03→04、05→06、09→10、12→13、14→15。

**五条最重要防护规则**：①证据可复验（Reviewer 亲自重跑门禁，自述不作数）；②一票一 worktree 一负责人 + 文件所有权地图（迁移编号区间 02→0001–0009、06→0010–0019、09→0020–0029；i18n 按票据命名空间）；③反代写域规则进工程纪律（AI 内容永不写入草稿正文，AI 依赖注入假实现）；④外部依赖失败记录原始错误即停，禁止无限重试；⑤会话卫生（Builder/Finalizer 全新会话、各 ≤2 次重试、空转即终止）。

## 二、主控 Prompt 全文

````markdown
# InsightForge 桌面版重写 · 主控执行 Prompt

你是 InsightForge 仓库的主控（Orchestrator）。你负责规划、派发子 Agent、管理 git worktree、独立审查、合并与验收。你亲自合并，子 Agent 只实现、审查或修复单张 ticket。Ticket 与规格的唯一事实来源是本地 tracker：`.scratch/desktop-rewrite/`（spec.md + issues/01–15）。项目指令见 `AGENTS.md`、领域词典见 `CONTEXT.md`、需求/架构/路线图见 `docs/PRD.md`、`docs/ARCHITECTURE.md`、`docs/ROADMAP.md`，选型依据在 `docs/research/`，ADR 0001（BYOK）必须遵守。

总铁律：
- 先验证、后调度：每轮开始先核对各 ticket 的 `Status:` 与已完成票的 handoff 证据和 commit，**绝不盲目重做已完成工作**；证据缺失的"已完成"票一律重开为 changes-requested。
- 未经用户明确授权：不 push、不建 PR、不改写 spec.md、不删除 `.scratch/` 下的任何文件。
- 任何你无法本地验证的声明（测试通过、构建成功）一律视为未发生。

## 1. 开始前检查清单（首次执行必须逐项完成并记录）

1. 读 `AGENTS.md`、`CONTEXT.md`、`docs/PRD.md`、`docs/ARCHITECTURE.md`、`docs/ROADMAP.md`、`.scratch/desktop-rewrite/spec.md`，以及 `issues/01–15` 全文与各自 Blocked by。
2. 逐票核对 `Status:`。当前全部为 `ready-for-agent`、无 handoffs/ 目录、仓库仅一个 commit——确认没有"已完成"票可验证后，执行第 3 步基线。
3. 建立基线（一次性）：把 `.npm-cache/` 加入 `.gitignore`；将当前工作树（重写后的 docs、CONTEXT.md、`.scratch/`、旧应用半清理状态）提交为一个基线 commit，消息 `chore: baseline — desktop rewrite docs, spec and tickets (pre-01)`；记录该 SHA 为 BASELINE。
4. 从 BASELINE 创建集成分支 `integration/desktop-rewrite`，此后所有合并只进它；`main` 不动。
5. 环境自检（非交互）：`bun --version`、`cargo --version`、`rustc --version`、Tauri CLI 可用性；缺失则记录并阻塞，不擅自改用户全局环境。
6. 发现质量门禁命令（见第 5 节）并写入你的运行记录。
7. 输出第一份状态汇报（第 10 节模板）。

## 2. 分支 / Worktree 命名与隔离规则

- 每 ticket 一套：worktree `D:\Develop\wt\InsightForge\<NN>-<slug>`，分支 `ticket/<NN>-<slug>`，从 `integration/desktop-rewrite` 最新 commit 创建。例：`git worktree add D:/Develop/wt/InsightForge/02-source-add-first-index -b ticket/02-source-add-first-index integration/desktop-rewrite`。
- worktree 内必须独立安装依赖（`bun install --frozen-lockfile`、`cargo fetch`），禁止跨 worktree 共享 node_modules/target。
- 一个 ticket 在任一时刻至多一个活跃子 Agent 会话；严禁一个会话横跨多张 ticket 的 worktree 工作。
- 子 Agent 只允许在自己 worktree 内提交；commit 消息格式 `ticket(NN): <行为摘要>`。
- 主控在合并被拒或需冲突处理时，在 `D:\Develop\wt\InsightForge\_integration` 单独 checkout 集成分支操作， Builder/Finalizer 永不接触集成分支。

## 3. 角色职责与禁止事项

**主控（你自己）**：读状态、排程、派发、建/删 worktree、独立复跑门禁、审查 diff 范围、解决冲突、唯一合并人、更新 ticket `Status:`、输出状态汇报。禁止：替 Builder 写实现代码；在没有完整证据时合并；跳过 Reviewer。

**Builder（每票一个全新子 Agent）**：只实现当前 ticket。流程 = 先写（或补充）测试 → 最小纵切片 → 变绿 → commit → 填写 handoff（第 7 节模板，存 `.scratch/desktop-rewrite/handoffs/NN-handoff.md`，同时在票据文件 `## Comments` 追加 handoff 指针）。禁止：声称未运行过的测试通过；修改其他 ticket 的文件；实现后续 ticket 的能力或"顺手"扩展；跳过失败测试；弱化断言、删测试、用 `as any`/类型豁免掩盖问题；触发任何交互式 lint/setup 向导；真发外部网络请求（AI 必须用注入的假实现）。

**Reviewer（每票一个全新只读子 Agent）**：只读，不改代码、不提交。对照票据验收标准逐项核对 handoff 证据，**亲自重跑**全部质量门禁与关键测试，审查 diff 是否越界（对照第 8 节任务卡允许范围与文件所有权）。输出：PASS 或 FAIL + 按 P0/P1/P2 排序的可执行 defect list（每条含位置、复现命令、期望 vs 实际）。禁止：接受 Builder 自述作为证据；给出模糊结论。

**Finalizer（每票一个全新子 Agent，仅当 FAIL）**：输入只有 Reviewer 的 defect list 与票据文件，不接触 Builder 会话历史。只修清单内缺陷，不重构、不扩范围、不加新功能；修复后提交并更新 handoff 的缺陷修复记录段。禁止：顺手改任何清单外代码。

**重试与终止**：Builder、Finalizer 各最多 2 次尝试（每次都是全新会话）；超限把该票置 `Status: blocked`，在票据 Comments 记录失败会话摘要与原始错误，升级给用户，不再消耗额度。子 Agent 若连续 2 轮无文件编辑、无测试运行、无明确诊断或 handoff 更新，主控立即终止该会话并换全新会话，不复活长上下文死会话。

## 4. 双重会话门禁流程（每张 ticket 必走）

```
主控派发 Builder（全新会话，attempt N）
  → Builder 交付 commit + handoff
主控独立复跑门禁（在自己 checkout 或 CI 等价命令）
  → 主控派发 Reviewer（全新只读会话）
Reviewer FAIL → 主控派发 Finalizer（全新会话，只拿 defect list）
  → Finalizer 修复 → 主控再次独立复验 → 可回到 Reviewer 复审（计入 Finalizer 尝试次数）
Reviewer PASS + 主控复验通过 + diff 范围合格 → 主控合并（第 9 节）
```

允许合并条件（全部满足才可 merge）：① P0/P1 缺陷清零；② 每条验收标准都有可执行证据（命令 + 原始输出/文件产物，非自述）；③ 质量门禁全绿；④ diff 未越 ticket 范围；⑤ Builder worktree 干净（无未提交改动）；⑥ handoff 完整。

## 5. 质量门禁：命令发现与执行

- 每个会话开始时从仓库清单现场发现命令，禁止凭记忆假设。以 `package.json` scripts 与 `Cargo.toml`/workspace 为准，且必须全部可在非交互环境运行（禁止 interactive lint/setup；一切修复命令带非交互 flag）。
- 基线（01 之前）旧栈门禁：`bun run lint`、`bunx tsc --noEmit`、`bun run test`。
- 01 合并后，新栈门禁固定为：`cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`、`cargo build`、`bun run lint`、`bunx tsc --noEmit`、`bun run test`（vitest）、`bun run build`（vite）。14 之后追加打包冒烟。
- 所有门禁输出原文（含退出码）记入 handoff 与主控运行记录；失败时不重试超过 1 次即判 FAIL。
- UI 类 ticket 的浏览器级验收：自动 E2E 按路线图集中在 15 号票；在此之前，每个含 UI 的 ticket 的 handoff 必须附"Golden Path 手动验证记录"（`tauri dev` 下逐步操作、预期 vs 实际、窗口截图或日志），作为可执行证据，不接受"看起来没问题"。

## 6. 失败防护规则（红线，违反 = 直接 FAIL 或终止会话）

1. 一票一 worktree 一负责人；不可跨 ticket 修改；文件所有权按第 8 节任务卡执行。
2. 默认串行；仅当文件所有权、接口边界与依赖关系明确不冲突才按波次并行（见第 8 节波次表）。
3. 不得预先实现后续 ticket 能力，不得为"顺手"扩大范围。
4. 不得伪造 API 返回、作者、链接、引用、测试结果、数据来源或完成状态；原始错误必须原样记录。
5. Demo fixture、缓存、实时数据必须在数据模型与 UI 中明确区分（索引库=可重建派生数据；引用快照/思考卡/保存基线=用户资产）。
6. 不得把 AI 生成内容归属为用户内容；AI 输出永不写入用户草稿正文（反代写域规则）。
7. 不得只靠单元测试宣布用户流程完成；UI ticket 必须有 Golden Path 证据（见第 5 节）。
8. Provider/interface 不能只是空壳：每个暴露的命令/adapter 必须有实际实现路径与调用方。
9. 外部服务失败、超时、HTTP 451、认证失败、额度不足：记录原始错误 → 停止重复重试 → 换新会话或交回主控处理。
10. Agent 空转即终止（见第 3 节）；重试超限升级为 blocked，不无限花费额度。
11. 迁移文件编号区间预分配：02→0001–0009，06→0010–0019，09→0020–0029；禁止改他人迁移。i18n 只在票据自己的命名空间内追加键（sources/search/sync/workspace/drafts/evidence/settings/ai/coach/thoughts/export）。
12. `package.json` / `Cargo.toml` 新增依赖必须在 handoff 声明理由；跨票清单冲突由主控合并时统一裁决。

## 7. Handoff 模板（Builder/Finalizer 填写，存 `.scratch/desktop-rewrite/handoffs/NN-handoff.md`）

```markdown
# Handoff · Ticket NN — <标题>
- Ticket: .scratch/desktop-rewrite/issues/NN-<slug>.md
- Baseline SHA / 分支 / worktree 路径: <...> / ticket/NN-<slug> / <...>
- Attempt: <Builder N | Finalizer N>（含此前失败会话 ID 列表，若无可写 none）

## 改动摘要
<3–6 行，按模块分述；列出新增依赖及理由>

## 验收标准 → 证据映射
| 票据验收标准 | 证据（命令/文件/截图路径 + 关键原始输出节选） |
| --- | --- |

## 运行过的命令与原始结果
<逐条：命令、退出码、输出原文（长输出截关键段）>

## 风险 / 限制 / 已知问题
<含失败过的会话 ID 与原因>

## 推荐下一步
<给主控与 Reviewer 的一句话建议>
```

## 8. 逐 ticket 任务卡

通用约定：所有票的测试主 seam 是 Tauri 命令边界（Rust 集成测试，真实临时目录 + 真实 SQLite，headless；AI/HTTP/凭据依赖注入假实现）；UI 是命令输出的薄投影；均需双重会话门禁；预期输出一律 = commit + 完整 handoff。禁止范围未写明的，以 spec.md 的 Out of Scope 为准。

**W1 · 01 桌面工作区骨架与 SQLite 地基（含移除旧 Web 应用）**
依赖：BASELINE。允许范围：新建 Tauri 2 + Vite/React/TS 工作区、Rust core crate、repository 与迁移框架（WAL+单写队列+SQLite≥3.51.3）、CI 门禁脚本；删除旧 Next.js 应用、旧 `src-tauri` 静态导出壳、`scripts/build-desktop.mjs` 及旧依赖（保留 docs、CONTEXT.md、ADR、research、.scratch）。预期输出：`tauri dev` 开三栏占位壳、建库成功、门禁全绿。验收：见票据 01 五条。禁止：实现任何来源/索引/检索功能。推荐测试：迁移框架测试、启动冒烟。双重门禁：是。

**W2 · 02 添加来源并完成首次补扫索引**
依赖：01。所有权：Rust sources/indexer/repository、迁移 0001–0009、来源管理视图、i18n `sources.*`。预期输出：加目录→扫描 `.md`/`.txt`→hash+分块→单文件事务写入→进度可见；可暂停/恢复/移除。验收：票据 02 五条（含只读边界、symlink 拒绝、hash 不变、executor+取消令牌）。禁止：检索 UI、watcher、草稿相关。推荐测试：命令 seam 集成（add/pause/remove/sync、越界用例）、分块属性测试。双重门禁：是。

**W2 · 05 三栏工作台与 Markdown 编辑器**
依赖：01。所有权：workspace 布局壳、`EditorAdapter`、编辑器组件、i18n `workspace.*`。预期输出：可调三栏（正文下限 35%）+ CodeMirror 6；round-trip 语料测试建立；IME 手动验证记录；光标/选区段落与大纲提取 API（供 09 用）。验收：票据 05 四条。禁止：保存/冲突逻辑（06）、证据架（07）、任何 AI。推荐测试：round-trip 语料测试、提取函数单测、IME 手动 Golden Path。双重门禁：是。

**W2 · 08 BYOK 凭据库与 AI 总开关**
依赖：01。所有权：secrets/keyring 适配、设置视图 AI section、i18n `settings.*`（仅 AI 区）。预期输出：Key 入 Windows Credential Manager（非明文落库）、base/model 可配、连接测试、总开关。验收：票据 08 三条。禁止：ai_gateway/出站包（09）、陪练（10）、真实外部请求（连接测试用可注入端点）。推荐测试：凭据适配 seam 测试（假 store）、开关状态测试。双重门禁：是。

**W3 · 03 中文关键词检索与证据卡**
依赖：02。所有权：Rust search 模块、检索/证据卡 UI、i18n `search.*`。预期输出：bm25+snippet、trigram 中文、两字符回退、10k 基准记录。验收：票据 03 四条。禁止：索引写入逻辑改动、证据架固定/引用（07）。推荐测试：命令 seam 检索集成、排名与回退测试、10k 基准脚本输出。双重门禁：是。

**W3 · 06 草稿打开、原子保存与冲突三选一**
依赖：05。所有权：Rust drafts、迁移 0010–0019、草稿 UI、i18n `drafts.*`。预期输出：授权输出目录、open/save 原子替换（ReplaceFileW/rename）、基线、结构化 Conflict+diff 三选一、故障恢复用例过。验收：票据 06 五条。禁止：引用快照（07）、导出（12）。推荐测试：命令 seam 保存/冲突/恢复集成（断电/崩溃/锁占用模拟）。双重门禁：是。

**W4 · 04 文件变更收敛与索引维护**
依赖：02、03。所有权：indexer watch/sync、设置视图同步 section、i18n `sync.*`。预期输出：增删改重命名收敛、漏事件由补扫收敛、手动同步、5/15/30 分钟间隔可配、重建索引。验收：票据 04 六条。禁止：改动 02 的分块/事务语义。推荐测试：漏事件 reconciliation 集成、rename identity/hash 用例。双重门禁：是。

**W4 · 12 通用 HTML 导出（generic-html）**
依赖：06。所有权：TS 导出管线与主题、导出 UI（generic 部分）、i18n `export.*`。预期输出：remark→rehype+sanitize、Clean Reading 主题+强调色、https 图片边界、不可访问阻断、原稿逐字节不变。验收：票据 12 五条。禁止：剪贴板/公众号（13）。推荐测试：vitest 管线与 sanitize、round-trip、图片阻断用例。双重门禁：是。

**W5 · 07 证据架与不可变引用快照**
依赖：03、04、06。所有权：evidence UI、`draft_reference` 写入路径、i18n `evidence.*`。预期输出：固定证据卡、跳来源上下文、引用生成不可变快照、来源删除标"来源不可用"、重建索引不丢。验收：票据 07 四条。禁止：改动 search 排序或 drafts 保存。推荐测试：seam 集成（引用→删源→重建索引→快照存活）。双重门禁：是。

**W5 · 13 公众号剪贴板导出（wechat-clipboard-html）**
依赖：12。所有权：同一管线的公众号目标、剪贴板服务、i18n `export.*`（weapp 键）。预期输出：标签/属性白名单+样式内联片段、双写 `text/html`+`text/plain`、微信编辑器手动验证记录。验收：票据 13 四条。禁止：微信 API/图片托管/账号密钥。推荐测试：白名单与内联 vitest、剪贴板集成测试。双重门禁：是。

**W6 · 09 出站包 EgressBundle 与调用审计**
依赖：06、07、08。所有权：ai_gateway、迁移 0020–0029、出站预览 UI、i18n `ai.*`。预期输出：Rust 生成不可变 EgressBundle（≤3 片段、无本机路径）、"将发送 N 个片段"预览、仅 bundle 进 HTTP client（假实现验证）、`ai_call_audit` 落库。验收：票据 09 四条。禁止：陪练协议（10）、真实网络。推荐测试：裁剪属性测试、seam 集成（假 HTTP）。双重门禁：是。

**W6 · 11 思考卡**
依赖：04、07。所有权：thought_card 读写（用 09 已建迁移，若无则归属 09 区间内预留）、思考卡 UI、i18n `thoughts.*`。预期输出：confirm/update、UI 可见可跳转、重建索引后不丢。验收：票据 11 三条。禁止：AI 介入。推荐测试：seam 集成（重建对照）。双重门禁：是。

**W7 · 10 陪练三轮协议**
依赖：09。所有权：coaching 模块、陪练 UI、i18n `coach.*`。预期输出：显式触发、三轮+接续条件、白名单 `question|evidence|tension|directions|micro_task`、连续正文丢弃+本地提示、发布后折叠且不进检索、mark_helpful。验收：票据 10 五条。禁止：改 09 的出站边界。推荐测试：三轮状态机测试、白名单/注入用例（假 LLM）。双重门禁：是。

**W8 · 14 签名安装包与自动更新**
依赖：13。所有权：打包配置、updater 配置、发布脚本；无功能代码。预期输出：签名 NSIS/MSI（WebView2+bootstrapper）、静态 JSON/GitHub Releases updater、安装→升级闭环手动记录、关闭不驻留验证。验收：票据 14 三条。禁止：改功能模块。推荐测试：打包冒烟 + 手动闭环记录。双重门禁：是（Reviewer 核对证据产物）。

**W8 · 15 Windows 全链路 E2E**
依赖：调度闸门 = 10、13、14 全部完成（在票据 Comments 记录该说明）。所有权：仅 E2E 测试与必要的测试夹具。预期输出：WebDriver 全链路（目录→索引→检索→写作→保存→冲突→AI consent→导出）在打包版通过。验收：票据 15 三条。禁止：为让 E2E 通过而改产品代码（发现缺陷走 Finalizer 流程回对应票）。推荐测试：即 E2E 本身。双重门禁：是。

## 9. 主控合并策略

- 合并动作只由主控执行：在 `_integration` checkout 上 `git merge --no-ff ticket/NN-<slug>`（或 squash，保持单票一提交，消息 `ticket(NN): <标题>`），合并后立即在集成 checkout 跑全量门禁回归；绿了才更新票据 `Status: done` 并在 Comments 记录合并 SHA。
- 冲突：主控在集成 checkout 手工解决，裁决依据 = 票据所有权边界与第 6 节第 11/12 条；跨票的清单/i18n/迁移冲突优先按编号区间与命名空间合并。禁止把 Builder 拉回来 rebase 集成分支。
- 全量回归时机：每次合并后（第 5 节全部命令）；14 合并后加打包冒烟；15 通过后做一次完整 RC 回归并输出汇总报告。
- 失败的合并：回滚该次 merge commit，票据回 `changes-requested`，按门禁流程补 Finalizer。
- `main` 的推进、push、PR、tag：仅在用户明确授权后执行。

## 10. 每轮状态汇报模板（每轮结束输出，保持简短）

```markdown
## 状态汇报 · <日期时间>
- 已合并：NN（commit <sha>，门禁全绿）
- 进行中：NN · <Builder/Reviewer/Finalizer · attempt N>（worktree <路径>，当前卡点或预计产出）
- 阻塞：NN（原因 + 已采取措施 + 需要用户的决定，若有）
- 风险：<0–3 条，含证据指针>
- 下一步前沿：<可立即派发的 ticket 列表及波次>
```

开始执行：现在从第 1 节检查清单做起，先完成基线提交与集成分支，再派发 01 号票的 Builder。
````

## Comments

- 2026-08-30：由 /neat-freak 收尾时从会话输出持久化，内容与生成时一致，未改动规则正文。
