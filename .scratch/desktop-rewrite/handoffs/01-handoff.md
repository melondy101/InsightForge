# Handoff · Ticket 01 — 桌面工作区骨架与 SQLite 地基
- Ticket: .scratch/desktop-rewrite/issues/01-workspace-foundation.md
- Baseline SHA / 分支 / worktree 路径: 48a4f67 / ticket/01-workspace-foundation / D:\Develop\wt\InsightForge\01-workspace-foundation
- Attempt: Builder 1（此前失败会话：none）

## 改动摘要
- **移除旧栈**（50667a0）：`git rm` 掉 `src/app`、`src/components`、`src/lib`、`src/utils`、旧 `src/i18n`、`public/`、`scripts/`（build-desktop/db-migrate）、`next.config.ts`、`drizzle.config.ts`、`vercel.json`、`components.json`、`.env.example`、postcss/tailwind、旧 `eslint.config.mjs`/`tsconfig.json`、旧 `src-tauri` 静态导出壳（保留 icons 与 app-icon.png）、旧 `.github/workflows/desktop-release.yml`。docs/、CONTEXT.md、AGENTS.md、.scratch、README/CHANGELOG/CONTRIBUTING/SECURITY 完整保留。
- **新前端**（5716f59）：Vite 8 + React 19 + TypeScript（~5.9）+ react-i18next（zh-CN 默认/en-US），三栏占位壳（侧栏/编辑器/证据架，`data-el` 定位属性），vitest 覆盖 i18n 资源（默认文案、语言切换、两语言键一致性）。
- **Rust core**（84dde72）：根 Cargo workspace + `crates/core`（crate 名 `insightforge-core`——与桌面壳解耦、平台无关测试可独立跑）。repository 模块：`Repository::open/open_with_migrations/read/write/journal_mode/applied_migrations/sqlite_libversion*`；WAL 固化（非 WAL 直接报错，绝不降级）；**单一写入队列**（专属 writer 线程 + mpsc 串行，写操作包在 `BEGIN IMMEDIATE` 事务内，失败整体回滚）；版本化迁移框架（`Migration`/`Migrator`/`MIGRATIONS`），簿记表 `schema_migrations` 为框架自建（幂等、非编号迁移），01 不写任何领域迁移，`MIGRATIONS = &[]`，编号 0001–0029 预留给 02/06/09。
- **Tauri 2 壳**（edbb7ed）：`src-tauri` 重建，devUrl=Vite 1420 / frontendDist=../dist；启动时在 `app_data_dir`（`%APPDATA%\app.insightforge.desktop`）建 `insightforge.sqlite` 并跑迁移，日志输出 SQLite 版本、journal_mode、已应用迁移数；capabilities 仅 `core:default`。
- **门禁与文档**（2012921 + 6a129ba）：`bun run gate` 一键串联全部门禁；新增 `.github/workflows/ci.yml`（Windows/Ubuntu 编译 core + 平台无关测试 + 前端门禁）；AGENTS.md 仅改两处（§四 提交前三绿 → 新栈等价物；§五 发布构建 → tauri dev/build）；README 运行/构建说明与桌面版技术栈更新，Web 版章节标注为历史存档。
- **新增依赖及理由**：
  - npm：`vite`/`@vitejs/plugin-react`（构建）、`typescript ~5.9.0`（**pin 原因：typescript-eslint 8.68 尚不支持 TS 7.0**，见风险）、`@types/react(-dom)`、`vitest`（AGENTS 测试约定）、`eslint`+`typescript-eslint`+`eslint-plugin-react-hooks`（lint 门禁）、`@tauri-apps/cli`（修复旧 `"tauri": "tauri"` 指向错误包名）、`@tauri-apps/api`（票据指定，后续命令 seam 的标准前端 API，当前占位壳尚未调用）、`react`/`react-dom`/`i18next`/`react-i18next`（复用）。
  - cargo：`rusqlite 0.40.2`（features=["bundled"]，**捆绑链接 SQLite 3.53.2 ≥ 3.51.3 铁律达成**）、dev-dep `tempfile 3`（集成测试真实临时目录）、`tauri 2`+`tauri-build 2`（壳）。移除旧壳的 `tauri-plugin-opener`；`serde/serde_json` 暂不引入（尚无业务命令）。
  - crate 命名理由：core 用 `insightforge-core`（kebab 包名/`insightforge_core` lib 名）表明它是与壳无关的应用核心；壳保留 `insightforge-desktop` 与 `app.insightforge.desktop` identifier 不变。

## 验收标准 → 证据映射
| 票据验收标准 | 证据（命令/文件/截图路径 + 关键原始输出节选） |
| --- | --- |
| 1. `tauri dev` 启动三栏壳；React+TS+Vite，无 Next.js/SSR/API routes | 截图 `handoffs/tauri-dev-window.png`（PrintWindow 窗口级截图：标题栏 InsightForge + 页头「织知 InsightForge」+ 侧栏/编辑器/证据架三栏）；dev 日志 `handoffs/evidence/tauri-dev.log`：`Running ...target\debug\insightforge-desktop.exe`；`Get-Process` 验证 `MainWindowTitle = InsightForge`；`package.json`/`vite.config.ts`/`index.html` 为新栈，仓库无 `src/app`、`next.config.ts` |
| 2. 首启建库 + 版本化迁移；WAL + 单写队列 + SQLite ≥ 3.51.3 固化在 repository 层 | 启动日志：`[insightforge] database ready: path=C:\Users\20145\AppData\Roaming\app.insightforge.desktop\insightforge.sqlite sqlite=3.53.2 journal_mode=wal applied_migrations=0`；该目录下存在 `insightforge.sqlite` + `-wal` + `-shm`；`crates/core/tests/repository.rs` 9 个集成测试（真实临时目录 + 真实 SQLite）全绿：建库、WAL PRAGMA、迁移应用与版本记录、重复打开幂等、迁移序列校验、8 线程×25 写经单写队列串行成功（200/200）、失败写整体回滚且队列存活、链接版本 ≥ 3051003 断言 |
| 3. 旧 Web 应用移除；文档/ADR/CONTEXT/调研保留 | commit 50667a0 删除清单（git show --stat）；`git ls-files` 无 next/drizzle/vercel/components.json/.env.example/scripts/*；`docs/`（含 adr/、research/）、`CONTEXT.md`、`AGENTS.md`、`.scratch/` 原样保留（仅 handoffs/ 与 issue Comments 为本次新增/追加） |
| 4. CI 编译 Rust core 并跑平台无关测试 | `.github/workflows/ci.yml`：windows-latest + ubuntu-22.04 矩阵 `cargo fmt/clippy -p insightforge-core/test/build -p insightforge-core --locked`；ubuntu 前端四门禁。**注：本地无法运行 GitHub Actions，未验证，见风险** |
| 5. 「提交前三绿」更新为新栈等价物 | commit 2012921：AGENTS.md §四 改为「cargo fmt/clippy/test/build + 前端 tsc/lint/test/build，一键 `bun run gate`」；§五 改为 `bun run tauri dev` / `bun run tauri build`；其余章节零改动（git diff 可查） |

## 运行过的命令与原始结果
> 本机 PATH 上有旧版独立 cargo 1.90（scoop rust-msvc shim）遮蔽 rustup stable 1.94。**以下所有 cargo 命令均在同一 bash 内前置 `PATH="/c/Users/20145/scoop/persist/rustup/.cargo/bin:$PATH"` 执行**，实际工具链：cargo/rustc/clippy 1.94.0。

1. **SQLite 版本探针**（临时小程序，rusqlite 0.40 bundled）— 退出码 0
   ```
   linked sqlite = 3.53.2 (number=3053002)   // ≥ 3051003（3.51.3）✓ 铁律达成
   ```
2. **TDD 红灯记录**（每个行为先红后绿，`cargo test -p insightforge-core`，均退出码 101）：
   - `error[E0432]: unresolved import insightforge_core::Repository`（曳光弹：建库）
   - `error[E0599]: no method named journal_mode`（WAL 固化）
   - `error[E0432]: unresolved import insightforge_core::Migration` + `no function open_with_migrations`（迁移框架）
   - `error[E0599]: no method named write/read`（单写队列）
   - 运行期红灯：`failed_write_rolls_back_partial_work` → `left: 1 / right: 0`（无事务时半截写入残留）；`queue_keeps_accepting_writes...` → `left: 2 / right: 1`
   - `error[E0599]: no function or associated item named sqlite_libversion_number`（版本断言）
3. `cargo fmt --check` — **退出码 0**（无输出）
4. `cargo clippy -- -D warnings`（workspace 全体）— **退出码 0**，`Finished `dev` profile ... in 4.25s`（期间修掉一处 `needless_question_mark`）
5. `cargo test`（workspace）— **退出码 0**
   ```
   test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; ... (crates/core 集成测试)
   ```
6. `cargo build`（workspace，含 Tauri 壳）— **退出码 0**，clean 全量 `Finished ... in 2m 13s`，增量 `Finished ... in 1.38s`
7. `bun run lint`（eslint .）— **退出码 0**（0 error / 0 warning）
8. `bunx tsc --noEmit` — **退出码 0**（TypeScript 5.9.3）
9. `bun run test`（vitest run）— **退出码 0**
   ```
   ✓ src/i18n/i18n.test.ts (3 tests) 11ms
   Test Files  1 passed (1);  Tests  3 passed (3)
   ```
10. `bun run build`（vite build）— **退出码 0**
    ```
    dist/index.html 0.40 kB │ dist/assets/index-*.css 0.69 kB │ dist/assets/index-*.js 240.71 kB
    ✓ built in 228ms
    ```
11. **`bun run gate`（一键门禁全链）— 退出码 0**（完整原始输出：`handoffs/evidence/gate-full.log`，串联 8 条门禁逐段通过）
12. **`bun run tauri dev`**（后台，日志 `handoffs/evidence/tauri-dev.log`）— **退出码 0（TAURI_DEV_EXIT=0）**
    ```
    Finished `dev` profile ... in 1m 53s
    Running `...target\debug\insightforge-desktop.exe`
    [insightforge] database ready: path=C:\Users\20145\AppData\Roaming\app.insightforge.desktop\insightforge.sqlite sqlite=3.53.2 journal_mode=wal applied_migrations=0
    ```
    退出后 `Get-Process insightforge-desktop` 计数 = 0，日志无 compile error / renderer error。
13. **窗口证据**：`Get-Process` 输出 `insightforge-desktop  InsightForge`；截图 `handoffs/tauri-dev-window.png`（1214x858，PrintWindow/PW_RENDERFULLCONTENT 窗口级捕获：三栏「侧栏 / 编辑器 / 证据架」+ 页头「织知 InsightForge」均可见）。

## 风险 / 限制 / 已知问题
- **本机工具链 PATH 陷阱（最重要）**：`where cargo` 命中的 scoop shim 指向独立安装的 cargo 1.90，而 `clippy-driver`/rustup stable 是 1.94.0。直接 `cargo clippy` 会 E0514；**`bun run tauri dev` 首次在无 PATH 前置下启动时，1.90 cargo 与 1.94 clippy 混编导致 rustc 崩溃（0xc0000409 STATUS_STACK_BUFFER_OVERRUN，indexmap 编译失败）**，`cargo clean` 后以 rustup 1.94 一致重跑成功。后续会话在本机必须前置 rustup 代理路径，或修复 scoop shim/卸载旧 rust-msvc。CI 与其他机器不受影响。
- **CI workflow 未验证**：本地无法执行 GitHub Actions，`.github/workflows/ci.yml` 语法与步骤未经实跑；已删除旧 `desktop-release.yml`（其依赖的 desktop:export 已不存在，tag 发布链路归票据 14 重建）。
- **截图过程隐私事故（已处置）**：首次全屏截屏捕获到用户屏幕上正在播放的无关视频内容，已立即删除该文件；改用 Win32 `PrintWindow`（PW_RENDERFULLCONTENT）仅捕获应用窗口矩形。最终入库截图只含应用窗口。
- `Repository::read` 与 `write` 共用同一 writer 线程（读也被串行化）——01 的保守正确实现；WAL 语义允许后续票据加只读连接池优化检索，不动公共 API。
- `write()` 自动包 `BEGIN IMMEDIATE` 事务：单个写操作即单事务（spec「单个 SQLite 事务内替换片段」的落点）。若未来某命令需要跨操作事务，需扩展 API（届时 TDD）。
- CSP 暂为 `null`（占位壳无任何命令/权限）；接入业务命令的票据应同步收紧。
- vitest 仅 3 个 i18n 测试（票据要求的最小真实门禁）；UI 层无测试（spec Testing Decisions 明示本阶段不为 UI 写测试）。
- `package.json` 改名 `insightforge`、版本 0.1.0；桌面版自 0.1.0 重新计数（spec Further Notes）。`.gitignore` 保留了少量无害旧条目（out/、.api-parked 等）。
- 失败会话记录：本 attempt 内除上述 tauri dev 首启崩溃（已恢复并留有记录）外，无中断/失败会话。

## 推荐下一步
主控可直接放行 02 号票在本结构上开工（迁移从 0001 `knowledge_source` 开始，`MIGRATIONS` 常量即注入点）；Reviewer 建议重点审 `crates/core/src/repository/mod.rs` 的错误模型与 `write()` 自动事务语义是否符合预期，并在合并前跑一次 `bun run gate` 复核。
