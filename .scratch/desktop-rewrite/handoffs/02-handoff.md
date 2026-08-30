# Handoff · Ticket 02 — 添加来源并完成首次补扫索引
- Ticket: .scratch/desktop-rewrite/issues/02-source-add-first-index.md
- Baseline SHA / 分支 / worktree 路径: 60c8699 / ticket/02-source-add-first-index / D:\Develop\wt\InsightForge\02-source-add-first-index
- Attempt: Builder 2（此前失败会话：attempt 1 模型请求失败崩溃，遗留 10 commit 已由主控 cargo test 复验，attempt 2 又独立复验）

## 改动摘要

**attempt 1 遗留工作审计结论（亲自逐文件复核，不采信其 commit 自述）**：行为合格，保持原样、零代码改动。10 个 commit 真实覆盖 5 条验收标准：迁移 0001–0003（仅此三个，未触碰 0010+）；分块纯函数（标题优先 → 段落与长度窗口打包 → 单行超窗硬切，含无损覆盖属性测试）；扫描边界（根规范化去 verbatim 前缀、`validate_rel_path` 拒绝全部 traversal 形态、bare/anchored 双 GlobSet ignore、Windows volume-serial+file-index identity、只读打开是知识库文件的唯一打开方式）；Sources 命令 seam（`sources_add/list/pause/remove/sync`，pause 带 bool，重活在 `Executor` 专门线程、每作业 `CancelToken`、事件经 sink）；增量语义（size+mtime 快路径 → hash 慢路径 → 单事务替换片段与 FTS）；测试全部断言公共接口可观察行为（事件序列、列表状态、DB 最终态、源文件 hash/mtime/size），无一耦合内部实现，可被内部重写存活。审计未发现需要 TDD 补齐的行为缺口，故 attempt 2 无新增实现代码。

**attempt 2 相对遗留状态的增量（3 个 commit，全部是归置/门禁/文档）**：
- `564c426` 归置 attempt 1 遗留物：**还原被 attempt 1 越界提交覆盖的 01 号票证据 `handoffs/evidence/gate-full.log`**（从 60c8699 恢复 01 原始内容；attempt 1 的门禁日志改存我方新文件，见下）；attempt 1 的未完成 Golden Path 截图（gp-01..03，原路径误为双层 `evidence/evidence/`）移入 `handoffs/evidence/attempt1/` 留档；其可复用 UI 驱动脚本（`handoffs/scripts/`：CDP 驱动 gp-driver.ts、PrintWindow 截图 capture-windows.ps1、步骤清单）收编入库。
- `7db0d11` eslint ignores 增加 `.scratch/**`：驱动脚本入库后 `eslint .` 扫到 gp-driver.ts 一处 `no-unused-vars`（attempt 1 声称"gate 全链通过"时该文件尚未入库，其声明未覆盖它）；tsc 的 include 本就不含 `.scratch`，两门口禁口径对齐。产品代码零改动。
- 本 commit：`handoffs/02-handoff.md` + 票据 Comments + attempt 2 全部证据（门禁日志、tauri dev 日志、GP 截图与步骤日志、源目录 hash 前后比对）。

**新增依赖及理由（attempt 1 引入，attempt 2 复核确认均必要）**：
- cargo（core）：`globset 0.4`（ignore globs 匹配）、`walkdir 2`（目录遍历，`follow_links(false)` 不跟随 symlink/junction）、`sha2 0.10`（内容 hash）、`uuid 1 v4`（来源 id）、`serde/serde_json`（SourceEvent/SourceSummary 序列化给前端 + ignore_globs 存 JSON）、dev-dep 沿用 `tempfile 3`；`windows-sys 0.60`（仅 Windows，features 最小化：GetFileInformationByHandle 取 volume serial + file index 作稳定 file identity）。
- cargo（壳）：`tauri-plugin-dialog 2` + npm `@tauri-apps/plugin-dialog 2.7.2`（系统目录选择对话框；capabilities 仅加 `dialog:default`，无任何通用 FS 权限）。bun.lock 相应仅此一条新增。

## 验收标准 → 证据映射

| 票据验收标准 | 证据（命令/文件/截图路径 + 关键原始输出节选） |
| --- | --- |
| 1. 添加来源（目录选择 + 命名 + 默认忽略规则）后补扫完成，进度经事件通道驱动 UI 展示 | Golden Path（本次实跑，日志 `evidence/gp2-steps.log`、dev 日志 `evidence/tauri-dev-02.log`）：侧栏进「来源管理」（截图 `evidence/gp2-01-sources-view.png`，默认忽略规则展示「.git、node_modules、…」）→ 命名「金路径笔记」+ 目录 `…\Temp\if02-gp-kb` → 「添加并索引」→ **索引进行中截图 `evidence/gp2-02-indexing-progress.png`：状态「索引中」、进度条、「索引进度 445/620」**（事件驱动实拍）→ 完成后状态「空闲」、文件数 `620`（driver step-16 `"620" (expected 620)`）、Completed 事件后进度条消失（step-17）。目录选择对话框走真实 dialog 插件：`evidence/gp2-win-1-选择文件夹.png`（PrintWindow 捕获的原生「选择文件夹」窗口）。事件序列 Started→FileIndexed×N→Completed 由 `crates/core/tests/sources.rs::add_runs_first_backfill_landing_fragments_and_fts_with_progress_events` 断言（真实临时目录 + 真实 SQLite） |
| 2. Markdown 优先按标题、其次按段落与长度窗口分块；单个 SQLite 事务内替换该文件的片段与 FTS 行 | 纯函数层 `chunking.rs` 7 测试：标题优先、祖先标题路径、段落整段保留不拆、单行超窗硬切可还原、无损覆盖属性测试（300 伪随机文档按行无损覆盖且每块 ≤ 窗口）。集成层：GP 实库验证 Markdown 按标题 2 块/纯文本 1 块、FTS 行与片段一一对应（`add_runs_first_backfill…`）；`sync_reindexes_only_the_changed_file` 断言变更文件三小节 → 3 片段、FTS 总数恰好 +1（旧行被替换非追加）；`repeated_sync_never_duplicates_fragments` 三轮 sync 恒 (2,3,3)。单事务由 01 的 `repo.write()` 自动 `BEGIN IMMEDIATE` 保证（upsert_file + replace_fragments 在同一 write 闭包），失败整体回滚行为 01 已测 |
| 3. 暂停 / 恢复 / 移除来源可用；移除后源文件原样保留 | GP 实跑：暂停 → 状态「已暂停」（step-21，截图 `evidence/gp2-04-paused.png`）→ 恢复 →「空闲」（step-25）→ 手动同步收敛（step-28 文件数仍 620）→ 移除（confirm 自动接受）→ 列表空（step-31，截图 `evidence/gp2-05-removed-empty.png`）。**GP 后实测源目录 623 个文件 sha256 前后完全一致**（`evidence/gp2-kb-hashes-before.sha` vs `gp2-kb-hashes-after.sha`，diff 为空）；DB 终态 4 张领域表全 0 行、migrations=1,2,3。集成测试 `pause_rejects_sync_and_resume_restores_it`（暂停时 sync 被拒 SourcePaused）、`pause_stops_running_backfill_and_resume_sync_converges`（400 文件进行中暂停，恢复+同步收敛 400）、`remove_cleans_index_but_keeps_source_files_intact`、`remove_while_indexing_waits_then_cleans`（进行中移除等待停止后清理） |
| 4. 对知识库目录的写入路径在 Rust 侧被拒绝；symlink 逃逸与 path traversal 用例全部拒绝；源文件内容 hash 前后不变（M0 第 4 项） | 写入路径：`open_read_only` 是知识库文件的唯一打开方式，`read_only_handle_rejects_writes_but_allows_reads` 断言经该句柄 write 被操作系统拒绝；集成 `backfill_and_sync_never_modify_source_files` 断言 add+sync×2 后每个源文件 hash/mtime/size 三不变（M0 第 4 项），GP 的 623 文件 hash diff 为空再证。逃逸：`junction_escape_outside_root_is_never_indexed`（真实 `mklink /J` 目录联接，file_count=1 且 FTS MATCH '逃逸标记' 为 0）、`symlink_file_escape_outside_root_is_rejected`（本机有特权时实跑，无特权时打印 SKIP 并由 junction 用例兜底，共用 `stays_within_root` 边界）。traversal：`validate_rel_path_rejects_traversal_and_absolute_cases`（13 种形态全拒）+ `tampered_traversal_rel_path_is_rejected_on_sync`（篡改 DB 注入 `../secret.md` 后 sync 不读取、FTS MATCH 'ZZYYXX' 为 0） |
| 5. 阻塞型扫描 / 解析 / hash / 写入运行在专门 executor 且带取消令牌，索引期间 UI 不卡顿 | `executor.rs`：固定工作线程阻塞 executor，`jobs_run_on_executor_threads_and_all_complete` 断言作业运行在非调用线程；`cancel_token_is_shared_across_clones`。命令路径零重活：`add` 落库后立即返回 `Indexing` 状态（测试 step-11→12 间 UI 即时响应）。GP 实证：620 文件索引期间（step-13 捕获 444/620）driver 的 CDP eval 往返 <60ms（10:58:24.930 点击 → 24.932 waitFor 命中 → 24.983 截图完成），UI 持续渲染进度无卡顿。暂停/移除经 `CancelToken` 在文件边界尽快停止（`pause_stops_running_backfill…`、`remove_while_indexing_waits_then_cleans`） |

## 运行过的命令与原始结果

> 工具链：cargo/rustc/clippy 1.94.0（rustup）。**每条 cargo/tauri 命令前均 `export PATH=/c/Users/20145/scoop/persist/rustup/.cargo/bin:$PATH`**（规避 scoop shim 1.90 与 Tauri 混编崩溃，见 01 handoff 风险项）。

1. `cargo fmt --check` — **退出码 0**（无输出）
2. `cargo clippy --workspace -- -D warnings` — **退出码 0**：`Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.75s`
3. `cargo test` — **退出码 0**（4 个测试二进制，独立复验主控结论）：
   ```
   running 18 tests ... test result: ok. 18 passed   ← core 单元（chunking 7 + scan 9 + executor 2）
   running 9 tests  ... test result: ok. 9 passed    ← 01 repository 回归
   running 3 tests  ... test result: ok. 3 passed    ← schema（迁移 + trigram）
   running 18 tests ... test result: ok. 18 passed   ← sources 集成（真实临时目录 + SQLite）
   ```
4. `cargo build` — **退出码 0**：`Finished `dev` profile … in 0.69s`
5. `bun run lint` — **退出码 0**（`.scratch` ignore 修复前为 1 error：gp-driver.ts `no-unused-vars`，见改动摘要）
6. `bunx tsc --noEmit` — **退出码 0**（无输出）
7. `bun run test` — **退出码 0**：`Test Files 2 passed (2); Tests 7 passed (7)`（progress 3 + i18n 4）
8. `bun run build` — **退出码 0**：`dist/assets/index-*.js 253.26 kB │ gzip: 80.50 kB，✓ built in 218ms`
9. **`bun run gate` 一键全链 — 退出码 0**，完整原始输出 `evidence/gate-full-02.log`（8 段门禁逐段通过；日志唯一含 "warning" 字样的行是命令回显里的 `-D warnings` 旗标，非告警）
10. **`bun run tauri dev`（后台，`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`）— 退出码 0（TAURI_DEV_EXIT=0）**，日志 `evidence/tauri-dev-02.log`：
    ```
    Finished `dev` profile ... in 0.71s
    Running `D:\...\target\debug\insightforge-desktop.exe`
    [insightforge] database ready: path=C:\Users\20145\AppData\Roaming\app.insightforge.desktop\insightforge.sqlite sqlite=3.53.2 journal_mode=wal applied_migrations=3
    ```
    无 compile error / renderer error。
11. **Golden Path（本次实跑）**：驱动 `bun ../scripts/gp-driver.ts gp-steps-02.json gp2`（步骤定义 `evidence/gp-steps-02.json`，逐条「预期 vs 实际」日志 `evidence/gp2-steps.log`，36 步全部符合预期，无 ERROR）。关键节选：
    ```
    step-13:eval -> {"status":"索引中","progress":"索引进度 444/620"}（索引进行中，UI 实时）
    step-16:eval -> "620" (expected 620)（补扫完成文件数）
    step-21:eval -> "已暂停" (expected 已暂停)
    step-25:eval -> "空闲" (expected 空闲)（恢复）
    step-28:eval -> "620" (expected 620)（重复同步后不变）
    step-31:eval -> true (expected true)（移除后列表空）
    ```
12. **移除后源目录完整性**：`find … -type f | wc -l` = 623（前后一致）；623 文件 sha256 逐文件比对 `diff` 为空（`evidence/gp2-kb-hashes-{before,after}.sha`）。
13. **DB 终态**（bun:sqlite 只读查询）：`knowledge_source/source_file/fragment/fragment_fts` 全部 COUNT=0；`schema_migrations` = 1,2,3。
14. **收尾**：应用退出后 `Get-Process insightforge-desktop` 无输出（进程数 0），日志无错误。

## 风险 / 限制 / 已知问题
- **attempt 1 崩溃背景与遗留物处置**：attempt 1 在交付前因模型请求失败崩溃，未写 handoff、未写票据 Comments；其 Golden Path 只走到第 12 步（打开目录对话框）即中断（`evidence/attempt1/gp-steps.log`），三张截图与驱动脚本由 attempt 2 收编入库（`evidence/attempt1/`、`handoffs/scripts/`）。attempt 1 还曾越界提交覆盖 01 号票已提交证据 `evidence/gate-full.log`，attempt 2 已按主控对 tauri-dev.log 的同一处置原则从 60c8699 还原 01 原始内容；01 号票的两份证据文件（tauri-dev.log、gate-full.log）在本次交付的工作树中均与 60c8699 一致，attempt 2 未再触碰。
- **GP 收尾的诚实说明**：应用最终关闭是我用于关闭原生对话框的 WM_CLOSE 探针意外同时命中主窗口所致（等效点 ×，进程退出码 0、日志无错误，属优雅退出，非崩溃）；所有 GP 断言在此之前已完成并留档。「原生对话框关闭后页面恢复交互」这一步因此未单独留证，但 window.confirm 对话框（同一交互路径）已在移除步骤实测自动接受。
- 原生「选择文件夹」对话框截图（`gp2-win-1-选择文件夹.png`）背景中含本机文件系统的普通目录名（应用自身弹层的真实环境），仅存于本地分支。
- `symlink_file_escape_outside_root_is_rejected` 在未授予 SeCreateSymbolicLinkPrivilege 的机器上会打印 SKIP 跳过；目录级逃逸由 junction 用例（无需特权）提供硬证据，两者共用同一 `stays_within_root` 检查。
- 暂停/移除的取消在文件边界生效（已开始的单文件解析不中断）；`remove` 等待补扫真正停止后才清理（`done` 通道 join）。
- 沿袭 01 的已知限制：`Repository::read`/`write` 共用单写队列（保守正确，检索票据可加只读连接池）；CSP 仍为 null（接入业务命令后建议 03 号票一并收紧）。
- 小项（不阻塞、留给后续票据顺手处理）：前端 AddForm 的 `busy` prop 恒为 false（表单不随添加动作禁用）；`sync`/`remove` 对不存在 id 的 SourceNotFound 分支无专门测试（pause 有）；`sources.nav.workspace` 键放在 sources 命名空间下（语义稍错位，i18n 键一致性测试已覆盖）。
- 新增 npm 依赖仅 `@tauri-apps/plugin-dialog`；bun.lock 相应一行，无其他改动。无 `as any`、无类型豁免、无删测试、无弱化断言。

## 推荐下一步
主控可放行 03（检索）——`fragment_fts` trigram 已就绪且有中文命中测试，`Repository` 读路径可直接复用；Reviewer 建议重点复核 `indexer.rs` 的快路径（size+mtime 跳过）与 04 号票 watcher/rename 语义的衔接，以及 `sources.rs` 的 running-map 并发语义；合并前跑一次 `bun run gate` 复核。
