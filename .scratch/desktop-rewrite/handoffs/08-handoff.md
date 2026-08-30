# Handoff · Ticket 08 — BYOK 凭据库与 AI 总开关

- Ticket: .scratch/desktop-rewrite/issues/08-byok-keystore.md
- Baseline SHA / 分支 / worktree 路径: 54edd41 / ticket/08-byok-keystore / D:\Develop\wt\InsightForge\08-byok-keystore
- Attempt: Builder 1（中断）→ Builder 2（中断）→ **主控代收尾**。两次 Builder 会话均因「Model request failed」基础设施故障中断（非工程阻塞）：attempt 1 留 2 commit（1893c59、0baffa9），attempt 2 留 1 commit（5e882a7）+ 全套 GP 截图/脚本后中断，均未及写 handoff。
- **Provenance 声明（必读）**：本 handoff 由主控（Orchestrator）基于**亲自复跑的门禁输出**与 attempt 2 遗留的证据文件整理；Builder 自述一律未采信，未经主控或 Reviewer 亲证的内容在文中标注「待 Reviewer 验证」。Review 环节已明示 Reviewer：本文件仅为指针，不构成证据本身。

## 改动摘要（依据 commit 与 diff 亲读）
- **secrets 模块**（crates/core）：`SecretStore` trait 适配层 + `KeyringStore`（keyring crate → Windows Credential Manager）+ 内存假 store 供测试注入；服务名约定见代码（service=`insightforge`）。
- **settings 服务**：迁移 **0004** `app_setting`（kv 表；主控授权使用 02 区间未用编号）；base/model/总开关等非敏感配置经 repository 单写队列落 SQLite；AI 总开关守门：关闭时 AI 命令返回结构化不可用状态（`ai_sample` 样例命令演示该语义，供 09/10 复用）。
- **连接测试**：`AiHttpClient` trait + `UreqAiHttpClient`（ureq）实现；测试全部注入假 client，零真实网络。
- **Tauri 壳**：注册 settings/ai 样例共 7 个命令，`KeyringStore` + `UreqAiHttpClient` 注入。
- **前端**：设置视图（侧栏第三入口）——base URL、model、API Key（password，**永不回显**，仅「已配置」徽章 + 清除按钮）、连接测试、总开关；`settings.*` i18n（zh/en 对称）；13+3 项投影测试；attempt 2 GP 实测发现「重开设置空表单保存误清配置」缺陷并 TDD 修复（`draftFromSnapshot` 纯函数，先红后绿 3 测，commit 5e882a7）。
- **新增依赖**：cargo `keyring`（凭据库）、`ureq`（可注入 HTTP client 的默认实现）；npm 无新增。理由：票据验收 1/2 直接要求。

## 验收标准 → 证据映射
| 票据验收标准 | 证据（命令/文件/截图 + 关键输出） |
| --- | --- |
| 1. Key 经 keyring 入 Windows Credential Manager；DB 与前端拿不到明文 | 假 store seam 契约测试 8 项（`cargo test` 8 passed，主控亲跑）+ 集成测试断言 DB 文件字节不含 key（14 passed 内，代码位置 `crates/core/tests/`——**待 Reviewer 逐条亲证**）；UI 侧 Key 仅显示「已配置（存于系统凭据库）」徽章不回显（截图 `evidence/08-gp-03-saved-configured.png`，主控亲自查验）；真实 Credential Manager 写入佐证截图（gp-03/04/10/12）——**代码级真实 keyring 冒烟待 Reviewer 执行** |
| 2. base / model 可配置，连接测试可用 | 本地 mock 端点（`scripts/mock-ai-server-08.ts`，127.0.0.1:18787）连接成功（截图 `08-gp-04-test-connection-ok.png`「连接成功」）；设置往返与重开回填（截图 `08-gp-12-reopen-backfilled.png`）；vitest 101 passed 含设置投影测试（主控亲跑） |
| 3. AI 总开关关闭时 AI 命令返回明确不可用，其余功能不受影响 | 集成测试断言关闭→不可用状态（25 unit + 14 integration 内，**待 Reviewer 亲证**）；UI 截图 `08-gp-06-toggle-off-unavailable.png`「AI 总开关已关闭」；来源管理与工作台不受影响（截图 `08-gp-07/08`）；主控亲跑 cargo test 77 passed（25+9+3+8+14+18）+ vitest 101 passed |

## 运行过的命令与原始结果（主控亲跑 @ HEAD 5e882a7）
- `cargo fmt --check` → exit 0
- `cargo clippy --workspace -- -D warnings` → exit 0（Finished dev profile）
- `cargo test` → exit 0；test result: 25 passed / 9 passed / 3 passed / 8 passed / 14 passed / 18 passed（六个有内容二进制，共 77；其余 0-test 二进制略）
- `cargo build` → exit 0
- `bun run lint` → exit 0（eslint 无输出）
- `bunx tsc --noEmit` → exit 0（无输出）
- `bun run test` → exit 0；Test Files 8 passed (8)；Tests 101 passed (101)
- `bun run build` → exit 0（仅 CodeMirror chunk 体积提示，非告警）

## 风险 / 限制 / 已知问题
1. **GP 步骤日志缺失**：attempt 2 留下 12 张截图、4 组驱动步骤 JSON 与 mock 服务器脚本，但步骤执行日志未落盘；截图经主控逐张查验为真实应用窗口（含「AI 总开关已关闭」「连接成功」等状态）。Reviewer 需自跑 GP 补齐此项。
2. **真实 keyring 冒烟未独立执行**：Builder 会话中断前未留下独立的 Credential Manager 读取回证；DB 无明文断言存在于测试中但需 Reviewer 亲证。
3. 两次 Builder 会话因基础设施故障中断的背景已记入 orchestrator-log（cb0ea6e 后续条目）。
4. 工作树内 `src-tauri/Cargo.toml` 存在 tauri dev 触发的行尾幻影改动（内容零差异），主控已还原；后续会话如再现直接 `git checkout --` 即可。

## 推荐下一步
Reviewer 以「本 handoff 仅为指针」为准绳从零亲证三条验收标准（重点：keyring 真实链路、DB 无明文断言、开关守门语义），并自跑一套 GP；通过则主控合并。
