# Release Readiness — 织知 InsightForge 桌面版

> 文档性质：**事实源（source of truth）**，记录当前 `main` 在 `3139156` 的真实状态。
> 本文件随本分支的进展持续更新；所有“已验证”结论都以可复跑命令与产物为依据，
> “未运行”**不会**被写成“通过”。
> 产品定位不可改变：本地优先、反代写、Tauri 2 + React/TS/Vite + Rust 核心 + SQLite/FTS5。

---

## 1. 基线（Baseline）

| 项 | 值 | 来源 / 命令 |
| --- | --- | --- |
| 基线 commit | `3139156` — `docs: reconcile desktop delivery status` | `git rev-parse HEAD` |
| 工作分支 | `codex/desktop-release-readiness`（自 main 切出） | 本任务 |
| 产品版本（构建产物 / package / CHANGELOG） | `0.1.0` | `tauri.conf.json` / `package.json` / `CHANGELOG.md` |
| ARCHITECTURE 文档版本 | `1.0.0-draft`（与构建版本不一致，见 §6） | `docs/ARCHITECTURE.md` |
| 链接 SQLite 版本（实测） | `3.53.2`（version_number `3053002`） | `rusqlite::version()`，bundled `libsqlite3-sys 0.38.2` |
| Rust | 1.90.0 | `rustc --version` |
| Node/Bun | Node 22.23.2 / Bun 1.3.11 | `node -v` / `bun -v` |
| 前端测试 | **119 passed** | `bun run test`（`vitest run`） |
| 前端 lint / tsc / build | **全绿** | `bun run lint` / `bunx tsc --noEmit` / `bun run build` |
| Rust 测试 | **全绿（core 100 + desktop 8）** | `cargo test` |
| Rust fmt / clippy | **全绿** | `cargo fmt --check` / `cargo clippy -- -D warnings` |

> 质量门禁运行环境：Windows 11 + cmd.exe/Git Bash 本地机器。**本机器无法执行
> 真实签名安装包、WebView2 CDP 端到端、联网 AI 请求、安装→升级链路**。

---

## 2. 验证状态四分类

- **A. 已由当前 main 验证**：在本地环境可运行命令复跑并产生产物。
- **B. 代码存在但未在真实 Windows 桌面验证**：逻辑完备，但需真实安装包/WebView2/签名才能确认。
- **C. 必须由用户提供外部条件才能完成**：签名证书、HTTPS updater endpoint、真实安装升级实测。
- **D. 历史文档已过时 / 与代码矛盾**：下文明确列出，已或待修正。

---

## 3. 逐项清单（通过标准 / 命令 / 证据产物 / 责任边界）

### 3.1 可由本地自动验证（分类 A）

| # | 能力 | 通过标准 | 命令 | 证据产物 |
| --- | --- | --- | --- | --- |
| A1 | 前端门禁 | lint/tsc/test/build 全绿 | `bun run lint && bunx tsc --noEmit && bun run test && bun run build` | 终端退出码 0 |
| A2 | Rust 门禁 | fmt/clippy/test/build 全绿 | `cargo fmt --check && cargo clippy -- -D warnings && cargo test && cargo build` | 终端退出码 0 |
| A3 | 来源只读边界 | symlink/junction 逃逸被拒、源文件 hash 不变 | `cargo test -p insightforge-core --test sources` | 19 tests |
| A4 | 索引-搜索时序契约 | `await_index_committed` 后才 `searchable` | `cargo test -p insightforge-core --test index_search_contract` | 3 tests |
| A5 | 迁移框架 + schema | 迁移严格递增；打开后迁移版本可观察 | `cargo test -p insightforge-core --test schema --test repository` | 12 tests |
| A6 | BYOK 守门与脱敏 | 无 Key/关闭/连接失败均明确降级，报告不含明文 | `cargo test -p insightforge-core --test settings` | 14 tests |
| A7 | Release 配置 fail-closed | 缺 pubkey/endpoint/thumbprint 时 release 构建 panic | `cargo test -p insightforge-desktop`（updater_tests）+ `build.rs` 编译期检查 | 8 tests |
| A8 | 冲突三选一（**本分支新增**） | keep_local / adopt_external / save_as_copy 行为正确 | `cargo test -p insightforge-core drafts` + 前端测试 | Rust + TS 测试（见 §4） |
| A9 | 草稿不再静默丢弃磁盘 .md（**本分支新增**） | list() 自动登记游离草稿 | `cargo test -p insightforge-core drafts` | Rust 测试 |
| A10 | 原子保存临时文件安全（**本分支新增**） | 唯一临时名 + 残留清理 | `cargo test -p insightforge-core drafts` | Rust 测试 |
| A11 | DB 打开完整性校验（**本分支新增**） | 损坏库返回类型化错误并备份 | `cargo test -p insightforge-core --test repository` | Rust 测试 |
| A12 | 诊断快照（脱敏）（**本分支新增**） | 含版本/平台/schema 版本/来源数/索引状态/特性开关，无 Key/正文 | `cargo test -p insightforge-core diagnostics` | Rust 测试 |
| A13 | 结构化事件日志（**本分支新增**） | 关键链路稳定事件名 + 结果，可滚动/限容 | `cargo test -p insightforge-core eventlog` | Rust 测试 |

### 3.2 必须在真实 Windows 桌面环境验证（分类 B）

| # | 能力 | 通过标准 | 验证方式 | 责任边界 |
| --- | --- | --- | --- | --- |
| B1 | 首次启动 → 选来源 → 首索 → 搜中文 | 进度可见、中文命中、searchable 门控生效 | 手动脚本 `scripts/verify-windows/*.ps1`（见本分支） | 需真实 WebView2 + 用户目录 |
| B2 | 外部编辑冲突真实三选一 | 真实改文件 → 保存弹出冲突 → 三种选择均按预期改写/保留 | 手动脚本 + 应用内操作 | 需真实文件系统事件 |
| B3 | 重启后状态一致 | 来源/草稿/引用/索引可用性/最近错误状态一致 | 手动脚本（关→开两次） | 需重启应用 |
| B4 | 导出通用 HTML 与公众号剪贴板 | 反代写约束不被绕过，剪贴板双写 | 手动脚本 + 应用内操作 | 需真实剪贴板 |
| B5 | WebView2 CDP 端到端探针 | 关键主路径经 CDP 驱动成功；无 CDP 时明确 BLOCKED，不伪 PASSED | `bun run test:e2e`（需 `INSIGHTFORGE_CDP_ENDPOINT`） | 需签名安装包或 dev 实例 + CDP |

> 注意：`e2e/golden-path.spec.ts` 当前**仅含占位/被注释选择器**，不可作为“Windows 安装包
> E2E 通过”的证据（与 `e2e/README.md` 一致）。本分支提供最小探针与手动脚本。

### 3.3 必须由用户提供外部条件（分类 C — 明确未做）

| # | 阻塞项 | 必须由用户提供 | 当前状态 |
| --- | --- | --- | --- |
| C1 | 代码签名证书 | Windows 代码签名证书（含 thumbprint） | 未提供；`build.rs` 在 release 缺 `INSIGHTFORGE_CERT_THUMBPRINT` 时 fail-closed |
| C2 | HTTPS updater endpoint | 可访问的 `https://.../updater.json` 托管 | 未提供；`tauri.conf.json` 当前为 dev 占位 `https://localhost/updater-dev.json` |
| C3 | updater 签名密钥对 | 与 endpoint 上架 manifest 对应的私钥 | 未提供；`build.rs` 在 release 缺 `INSIGHTFORGE_UPDATER_PUBKEY` 时 fail-closed |
| C4 | 安装 → 升级 E2E | 真实安装旧版并升级到新版 | 未执行；依赖 C1–C3 与真实环境 |

> 这些项是**用户的待办**，本分支不伪造、不绕过、不标记为已完成。
> 拿到 C1–C3 后，按 `docs/RELEASE-RUNBOOK.md` 即可构建/签名/托管/安装/升级。

---

## 4. 本分支已落地能力（与 §3.1 A8–A13 对应）

| 能力 | Rust 行为测试 | 前端/集成测试 | 关键文件 |
| --- | --- | --- | --- |
| 冲突三选一 | `drafts::resolve_conflict` 三个分支用例 | `useDrafts` 解析完整 Conflict（含 externalContent）+ 三个动作 | `crates/core/src/drafts.rs`, `src/drafts/{api,useDrafts,DraftsView}.tsx` |
| 游离草稿登记 | list() 自动 upsert 磁盘 .md | — | `crates/core/src/drafts.rs` |
| 临时文件安全 | 唯一临时名 + 启动/列表时清理残留 | — | `crates/core/src/drafts.rs` |
| 开库完整性 | 写入垃圾 → `RepositoryError::Corrupt` + `.corrupt.bak` | — | `crates/core/src/repository/mod.rs` |
| 诊断快照 | `Diagnostics::collect` 脱敏快照（注入 now/版本） | 渲染 + 复制/导出 JSON | `crates/core/src/diagnostics.rs`, `src/diagnostics/*` |
| 事件日志 | `EventLog` 环形缓冲 + 文件滚动 | — | `crates/core/src/eventlog.rs` |

> 每个能力均先写失败测试 → 最小实现 → 转绿；任意时刻不超过一个失败测试。

---

## 5. 已知未验证／历史过期声明（分类 D）

- **D1（已修正）**：`CONTRIBUTING.md` §4 写“核心逻辑放在 `src/lib/zhizhi/*.ts`”，
  实为 `crates/core/src/*.rs`（与 AGENTS.md 一致）→ 已修改。
- **D2（已修正）**：`docs/ARCHITECTURE.md` 头部版本写为 `1.0.0-draft`，与构建产物版本
  `0.1.0` 不一致 → 已改为明确“目标版本 1.0.0；当前构建版本 0.1.0”。
- **D3（澄清）**：`ROADMAP.md` 2026-09-02 状态称“M1–M3 与双导出已合入 main”，
  但**冲突三选一后端原本未实现**（前端 `resolveConflict` 为 noop）。本分支已补齐，状态已更新。
- **D4（未做，属预期）**：`e2e/golden-path.spec.ts` 长期为占位，不能作为 E2E 通过证据
  （`e2e/README.md` 已声明）。本分支提供最小探针 + 手动脚本，不伪造 PASSED。

---

## 6. 责任边界（Responsibility Boundary）

- **本任务负责**：本地数据可靠性、可观测诊断、可复跑验证路径、发布工程 fail-closed 准备、
  文档准确性。
- **用户/外部负责**：C1–C4（签名证书、HTTPS endpoint、签名密钥、安装升级实测）、
  真实联网 AI 调用、真实个人数据目录下的长期稳定性观察。
- **不负责 / 不做**：Next.js/SSR/Web API route/Postgres/旧兼容层/云端依赖恢复；
  擅自发布、push、创建 PR、伪造签名证书或更新端点、伪造安装升级证据。

---

## 7. 复跑记录（Run Log）

| 日期 | 动作 | 结果 | 备注 |
| --- | --- | --- | --- |
| 2026-09-02 | 基线门禁全跑 | 全绿（前端 119 / Rust 全绿） | 发现 updater 测试偶发 flaky，已修复（见 git log） |
| 2026-09-02 | SQLite 版本实测 | 3.53.2 | 满足 ARCHITECTURE “≥3.51.3” 要求 |
