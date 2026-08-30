
### Ticket 01 · 桌面工作区骨架与 SQLite 地基（含移除旧 Web 应用）
- Builder attempt 1（agent_cdba17c5）：交付 7 个 commit（50667a0..7a41dee），SQLite 3.53.2 ≥ 3.51.3，Rust 9 测试 + 前端 3 测试全绿，tauri dev Golden Path 有日志+窗口截图证据。
- 主控独立复验：8 门禁全绿（fmt/clippy/test 9 passed/build + lint/tsc/test 3 passed/vite build）；diff 范围合格（AGENTS.md 仅 §四/§五，.scratch 仅 01 handoffs+Comments，spec.md 零改动）。
- Reviewer（agent_2cccbf08）：VERDICT PASS。P0/P1=0；P2×3 非阻塞（bun.lock 根名 "nextjs-template" 系 baseline 既有残留，frozen-lockfile 不受影响；writer 闭包 panic→队列关闭为设计取舍；read 共用单写线程为 01 保守实现）。证据交叉印证：测试二进制与 dist 产物哈希一致。
- 合并：7b7efaa `git merge --no-ff ticket/01-workspace-foundation`；合并后集成 checkout 全量 8 门禁回归全绿（bun install --frozen-lockfile exit 0）。
- 遗留跟进：P2-1 bun.lock 根名在下次触碰 package.json 的票据顺手修正。

### 事故记录 · 02 Builder attempt 1 会话崩溃（2026-08-30）
- 派发 02 Builder 后模型请求失败，子会话在交付 handoff 前崩溃；遗留 10 个 commit（c6c0bc0..558b9d3：迁移 0001–0003、分块纯函数、扫描边界、命令 seam、增量语义、暂停/恢复/移除、只读边界、Tauri 注册、来源管理视图+i18n）。
- 主控处置：还原其越界覆盖的 01 号票证据文件 handoffs/evidence/tauri-dev.log（git checkout）；cargo test 复验遗留 commit 真实全绿（18+9+3+18 passed）；未跟踪的 Golden Path 截图/脚本目录留给 attempt 2 归置。
- 决定：不重置分支；派发全新 Builder 会话 = attempt 2（末次 Builder 额度），任务为审计遗留工作 + TDD 补全 + 自跑全部门禁 + 自做 Golden Path + 撰写 handoff 02。若失败 → ticket 02 置 blocked 并升级用户。

### Ticket 02 · 添加来源并完成首次补扫索引（2026-08-30）
- attempt 1 崩溃事故见上；attempt 2（agent_0fcb1fad）审计遗留 10 commit 为行为合格零缺口，补 3 commit（归置遗留物/还原 01 被越界覆盖的 gate-full.log/eslint ignores .scratch）+ handoff + 36 步自跑 Golden Path（620 文件索引、移除后 623 文件 sha256 无 diff）。
- 主控独立复验：8 门禁全绿；13 commit 全 `ticket(02):`；01 证据与 60c8699 逐字节一致；迁移仅 0001–0003。
- Reviewer（agent_3fbabf2e）：PASS。本人重跑 GP 32/32（606 文件 hash 无 diff、integrity_check ok）、junction 逃逸真实拒绝、单事务替换代码级确认。P2×4 非阻塞。
- 合并：c5e0188 --no-ff；集成 8 门禁回归全绿。
- 后续跟进（P2，非阻塞）：gp-driver 加 vite-connected 守卫（05/03 做 GP 时顺手）；SourcesView busy；sources.nav.workspace 键；sync/remove NotFound 测试。
- 波次决定：W2 串行执行 02→05→08（三票共享命令注册与壳布局集成点，串行避免冲突）；05 先于 08 因其体量更大且解锁 06→12→13→14 发布链。

### Ticket 05 · 三栏工作台与 Markdown 编辑器（2026-08-30）
- Builder attempt 1（agent_204f01f3）：6 commit（b206feb..790852c）。EditorAdapter 窄接口 + CodeMirror 6；round-trip 语料逐字节断言（BOM/EOL 还原策略，TDD 红灯抓到 setValue 重设基线缺陷）；提取纯函数 18 测；三栏 35% 硬钳制；CDP imeSetComposition 真实组合输入 570 字验证（残余风险：OS 级 IME 未覆盖，15 号票复核——已在 handoff 声明）。
- 主控独立复验：8 门禁全绿（Rust 48 不变、前端 84）；范围干净、01/02 证据零改动。
- Reviewer（agent_3dbaf602）：PASS。亲手加 4 个新语料用例 + 变异检验（禁用 getValue 快路径→混合行尾用例立即红）；对称方向 35% 约束补测；Ctrl+Z 撤销链逐级验证。P2：clampPaneWidths 负侧栏宽度（真实窗口实证、700px 自愈）、openDocument 可被撤销到空文档（当前不可达，06 接线时处理）、语料计数 19→实为 18。P3：handoff 归因小误、建议语料加栅栏内 # 行。
- 合并：787bc76 --no-ff；集成 8 门禁回归全绿。
- 随单携带至 06：P2-1 负宽度钳制修复+边界测试；P2-2 openDocument 重建 state/isolate history 设计决定。
