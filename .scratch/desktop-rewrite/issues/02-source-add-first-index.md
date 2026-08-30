# 02 — 添加来源并完成首次补扫索引

**What to build:** 用户把一个本地文件夹添加为命名来源后，应用在后台扫描其中的 `.md` / `.txt` 文件，完成 hash 与分块并写入片段与 FTS 索引，索引进度实时可见；来源可暂停、恢复、移除（移除只清理索引记录，绝不动源文件）。知识库目录对应用严格只读。

**Blocked by:** 01 — 桌面工作区骨架与 SQLite 地基

**Status:** done (merged c5e0188)

- [x] 添加来源（目录选择 + 命名 + 默认忽略规则）后补扫完成，进度经事件通道驱动 UI 展示
- [x] Markdown 优先按标题、其次按段落与长度窗口分块；单个 SQLite 事务内替换该文件的片段与 FTS 行
- [x] 暂停 / 恢复 / 移除来源可用；移除后源文件原样保留
- [x] 对知识库目录的写入路径在 Rust 侧被拒绝；symlink 逃逸与 path traversal 用例全部拒绝；源文件内容 hash 前后不变（M0 第 4 项验证折叠于此）
- [x] 阻塞型扫描 / 解析 / hash / 写入运行在专门 executor 且带取消令牌，索引期间 UI 不卡顿

## Comments

- **2026-08-30 · Builder attempt 2**：交付完成。attempt 1（同票首次会话）因模型请求失败崩溃，遗留 10 commit（60c8699..558b9d3）未写 handoff；attempt 2 逐文件审计其测试与实现——5 条验收标准均有真实行为覆盖（公共接口断言、真实临时目录 + SQLite、抗内部重写），**零代码缺口、保持原样**，仅归置遗留物与门禁口径（564c426、7db0d11）并补齐文档与证据。门禁 8 条 + 一键 `bun run gate` 全绿（cargo test 18+9+3+18）；tauri dev Golden Path 本次实跑全过：620 文件索引、进度事件驱动 UI（实拍「索引进度 445/620」）、暂停/恢复/同步/移除、移除后 623 个源文件 sha256 前后一致、DB 领域表清零。证据与细节：`.scratch/desktop-rewrite/handoffs/02-handoff.md`。
- 2026-08-30 Orchestrator：attempt 1 会话崩溃（交付前中断，遗留 10 commit 经 cargo test 复验真实）；attempt 2 全新会话审计补全（+3 commit）。Reviewer 独立复验 PASS（P0/P1=0，P2×4 非阻塞：gp-driver 缺 vite-connected 守卫、SourcesView busy 恒 false、sources.nav.workspace 键语义错位、sync/remove NotFound 分支无测试）。合并条件①–⑥全满足，--no-ff 合入 c5e0188，集成 8 门禁回归全绿。
