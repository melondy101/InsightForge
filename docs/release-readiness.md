# Release Readiness — 织知 InsightForge

> 文档性质：**事实源（source of truth）**，记录分支 HEAD 的真实状态与验证边界。
> 本文件随分支进展持续更新；所有“已验证”结论都以可复跑命令与产物为依据，
> “未运行”**不会**被写成“通过”。
> 产品定位不可改变：本地优先、反代写、Tauri 2 + React/TS/Vite + Rust 核心 + SQLite/FTS5。

---

## 1. 基线（Baseline）

| 项 | 值 | 来源 / 命令 |
| --- | --- | --- |
| 基线 commit | `300b6b5` — `chore(repo): park one-off task-line artifacts under .scratch instead of docs`（其后的提交只动文档与检索片段渲染，不改命令面） | `git rev-parse --short HEAD` |
| 工作分支 | `feat/ui-v1-implement`（自 `main` 切出；**全部 commit 仍在本地，未 push**） | `git branch --show-current` |
| 历史基线 | `3139156`（`codex/desktop-release-readiness` 分支的 Phase A–F 收尾，已并入本分支线） | `docs/HANDOFF.md` §一 |
| 产品版本（构建产物 / package / CHANGELOG） | `0.1.0` | workspace `Cargo.toml` / `package.json` |
| ARCHITECTURE 文档版本 | `1.0.0-draft`（与构建版本不一致，见 §5 D2） | `docs/ARCHITECTURE.md` |
| 链接 SQLite 版本（实测） | `3.53.2`（version_number `3053002`） | `rusqlite::version()`，bundled `libsqlite3-sys 0.38.2` |
| Rust | 1.90.0 | `rustc --version` |
| Node/Bun | Node 22.23.2 / Bun 1.3.11 | `node -v` / `bun -v` |
| 前端测试 | **177 passed（21 文件）** | `bun run test`（`vitest run`） |
| 前端 lint / tsc / build | **全绿** | `bun run lint` / `bunx tsc --noEmit` / `bun run build` |
| Rust 测试 | **185 passed** | `cargo test --workspace` |
| Rust fmt / clippy | **全绿** | `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets -- -D warnings` |
| 界面实测 | 9 个视图 × 桌面/移动两种视口，真实浏览器跑通，横向溢出 0 | Playwright（手动，非 CI） |

> 质量门禁运行环境：Windows 11 + cmd.exe/Git Bash 本地机器。**本机器无法执行
> 真实签名安装包、WebView2 端到端、联网 AI 请求、安装→升级链路**（E2E 规格已就绪但受探针门控为 BLOCKED）。

---

## 2. 验证状态四分类

- **A. 已由当前 main 验证**：在本地环境可运行命令复跑并产生产物。
- **B. 代码存在但未在真实 Windows 桌面验证**：逻辑完备，但需真实安装包/WebView2/签名才能确认。
- **C. 必须由用户提供外部条件才能完成**：updater 私钥托管、仓库推送（决定清单端点可达）、真实安装升级实测。
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
| A7 | Release 配置 fail-closed | release 缺真实 endpoint 即 panic；配置里不许残留 `${env.}` 占位；公钥必须是真 minisign key | `cargo test -p insightforge-desktop`（`updater_tests`）+ `build.rs` 编译期检查 | 7 tests |
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
| B5 | WebView2 端到端探针 | `e2e/golden-path.spec.ts` 用真实 `data-el` 选择器与断言覆盖 7 条黄金路径；仅当 `INSIGHTFORGE_E2E_RUN=1` 且本机已装 WebView2（注册表探针）时运行，否则 `test.skip` 为 BLOCKED，**不伪 PASSED** | `bun run test:e2e`（需 `INSIGHTFORGE_E2E_RUN=1` + WebView2） | 需真实 Windows 11 + WebView2 环境 |

> 注意：`e2e/golden-path.spec.ts` 当前已用真实 `data-el` 选择器与断言实现 7 条黄金路径，但受
> `INSIGHTFORGE_E2E_RUN` + WebView2 注册表探针门控——本机无 WebView2 时整体 BLOCKED，
> 故**不可作为“Windows 安装包 E2E 通过”的证据**；真实验收见 `e2e/manual/verify.ps1`（与 `e2e/README.md` 一致）。

### 3.3 必须由用户提供外部条件（分类 C — 明确未做）

| # | 阻塞项 | 必须由用户提供 | 当前状态 |
| --- | --- | --- | --- |
| C1 | 代码签名证书（**已降级为可选**） | Windows 代码签名证书（含 thumbprint） | 未提供；缺省时 `build.rs` 只 warn，安装包不签名 → 首次下载有 SmartScreen 提示，updater 校验不受影响 |
| C2 | updater 清单端点 | 推送到 GitHub 并开出 Release | release.yml 已把端点定为 `.../releases/latest/download/latest.json`（构建期注入）；仓库未推送前该地址不存在 |
| C3 | updater minisign 私钥 | 把私钥内容存进仓库 secret `TAURI_SIGNING_PRIVATE_KEY` 并离线备份 | 密钥对已生成、公钥已进仓；**私钥目前只在构建机 temp 目录**，缺它 release.yml 第一步即退出 |
| C4 | 安装 → 升级 E2E | 真实安装旧版并升级到新版 | 未执行；依赖 C2/C3 与真实环境 |

> 这些项是**用户的待办**，本分支不伪造、不绕过、不标记为已完成。
> 逐项做法与失败症状见 `docs/RELEASE-RUNBOOK.md` §0–§2。

---

## 4. 本分支已落地能力（与 §3.1 A8–A13 对应）

| 能力 | Rust 行为测试 | 前端/集成测试 | 关键文件 |
| --- | --- | --- | --- |
| 冲突三选一 | `drafts::resolve_conflict` 三个分支用例 | `useDrafts` 解析完整 Conflict（含 externalContent）+ 三个动作 | `crates/core/src/drafts.rs`, `src/drafts/{api,useDrafts,DraftsView}.tsx` |
| 游离草稿登记 | list() 自动 upsert 磁盘 .md | — | `crates/core/src/drafts.rs` |
| 临时文件安全 | 唯一临时名 + 启动/列表时清理残留 | — | `crates/core/src/drafts.rs` |
| 开库完整性 | 写入垃圾 → `RepositoryError::Corrupt` + `.corrupt.bak` | — | `crates/core/src/repository/mod.rs` |
| 诊断快照 | `build_snapshot` 脱敏快照（注入 now/版本） | 渲染 + 复制/导出 JSON | `crates/core/src/diagnostics.rs`, `src/diagnostics/*` |
| 事件日志 | `EventLog` 环形缓冲 + 字段级脱敏（`MUST_NOT_LOG_KEYS`） | — | `crates/core/src/diagnostics.rs` |

> 每个能力均先写失败测试 → 最小实现 → 转绿；任意时刻不超过一个失败测试。

### 4.1 2026-09-19 一轮：ADR 0002 的客户端侧 + 技术债清偿

| 能力 | 机械约束在哪 | 关键文件 |
| --- | --- | --- |
| 三类 runtime 的命令通道（tauri / http / fake） | `src/native/command-client.test.ts`；未配置浏览器 fail-fast `runtime_unconfigured` | `src/native/command-client.ts` |
| 版本取自单一真源，不再直连 Tauri API | 同上 + CI 的 tag/Cargo/package.json 一致性步骤 | `CommandClient.appVersion()` |
| `SyncBundle`：同步载荷结构上带不了绝对路径 | `crates/core/tests/sync_bundle_contract.rs`（5 条，逐字节断言） | `crates/core/src/egress.rs` |
| 设备登记与撤销（撤销不可逆） | `crates/core/tests/devices_contract.rs`（11 条） | `crates/core/src/devices.rs` |
| 陪练前端入口（三轮协议、无「补全」按钮） | `src/coach/api.test.ts` 锁命令名与参数键；`coach_contract.rs` 8 条 | `src/coach/*`、`App.tsx` |
| 同步间隔真正生效（5/15/30，默认 15） | `sync_interval_contract.rs`（11 条）+ `store.rs` 断言 KV 前缀在 SELECT 里 | `crates/core/src/sources.rs` |
| 每个 Rust 错误码都有本地化文案 | `src/ui/error-text.test.ts` + `i18n.test.ts`（双语键对等、静态 `t()` 缺键即红） | `src/ui/error-text.ts`、`src/i18n/locales/*` |
| 样式双向契约（用到的类要有定义，样式表不留死规则） | `src/styles.test.ts` | `src/styles.css` |
| 网页「部署即更新」 | `deploy-web.yml`：缺 `INSIGHTFORGE_WEB_API_BASE` 即失败 + 产物内断言 | `.github/workflows/deploy-web.yml` |

仍缺的只有服务端本体：`insightforge-server`、`sync.push/pull` 发送方、设备管理界面。

---

## 5. 已知未验证／历史过期声明（分类 D）

- **D1（已修正）**：`CONTRIBUTING.md` §4 写“核心逻辑放在 `src/lib/zhizhi/*.ts`”，
  实为 `crates/core/src/*.rs`（与 AGENTS.md 一致）→ 已修改。
- **D2（已修正）**：`docs/ARCHITECTURE.md` 头部版本写为 `1.0.0-draft`，与构建产物版本
  `0.1.0` 不一致 → 已改为明确“目标版本 1.0.0；当前构建版本 0.1.0”。
- **D3（澄清）**：`ROADMAP.md` 2026-09-02 状态称“M1–M3 与双导出已合入 main”，
  但**冲突三选一后端原本未实现**（前端 `resolveConflict` 为 noop）。本分支已补齐，状态已更新。
- **D4（已澄清）**：`e2e/golden-path.spec.ts` 早期为占位；本分支已改写为真实 `data-el` 选择器 + 断言，
  但仍受 `INSIGHTFORGE_E2E_RUN` + WebView2 探针门控，无 WebView2 时明确 BLOCKED，不伪造 PASSED
  （见 `e2e/README.md`）。安装→升级实测仍依赖 C1–C3 与真实环境，未执行。
- **D5（已修正 2026-09-19）**：`SECURITY.md` 整篇描述的是**已冻结的旧 Web 版**——drizzle/Postgres、
  Next.js 16.3.0、`/api/discover` 等三条代转路由、`src/lib/llm/*`、localStorage 存 key。
  这些路径与依赖在当前仓库里都不存在（`ls src/lib` 无此目录，`package.json` 无 next/drizzle），
  其中「API key 走 `process.env` 或浏览器 localStorage」更与铁律「密钥只进 OS 凭据库」直接冲突。
  已改写为桌面 + ADR 0002 的当前基线，旧版审查结论不再冒充现状。
- **D6（已修正 2026-09-19）**：`CONTRIBUTING.md` §8 同样残留「代转路由的 BYOK key 仅在单次请求内存中使用」，
  已改为当前的凭据模型。

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
| 2026-09-03 | 全门禁复跑（Phase A–F 收尾） | 全绿（前端 127 / Rust 123；clippy `-D warnings` / fmt / build 通过） | 新增冲突三选一、孤儿草稿登记、临时文件清理、DB 完整性降级、脱敏诊断快照/事件日志、E2E 探针与发布 Runbook |
| 2026-09-19 | 全门禁复跑（ADR 0002 客户端侧 + 技术债清偿） | 全绿（前端 177 / Rust 185；`tsc` 0 错、`eslint` 0 警、`vite build`、`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`） | SyncBundle、devices、部署即更新、sync_interval 生效、coach 前端入口、错误码文案、样式双向契约、版本号收进 seam；工作树未跟踪归 0 |
| 2026-09-19 | 知识库洁癖审查（文档 ↔ 代码对账） | 全门禁重跑通过；真浏览器实测检索卡片（桌面/移动两视口，无溢出、无脚本执行） | 修掉检索证据卡的 HTML 注入面并加全仓守门；`SECURITY.md` 从旧 Web 版重写为当前基线；ARCHITECTURE / README / CONTEXT / CONTRIBUTING / CHANGELOG / HANDOFF / Runbook 全部对齐 |
