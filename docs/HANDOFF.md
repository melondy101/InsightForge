# 织知 InsightForge · 桌面版稳定化 + 1.0 发布就绪 · 验收交接

> 分支：`codex/desktop-release-readiness`（自 `main` 切出）
> 状态：全部门禁通过，**本地完成、未在远端 push / 未建 PR / 未发布 / 未签名 / 未触发更新**。
> 范围遵循产品定位：本地优先、反代写（AI 只给问题/骨架/证据，血肉由创作者写）。

---

## 一、本次交付概览（Phase A–F）

| Phase | 主题 | 关键产物 | 提交 |
|-------|------|----------|------|
| A | 基线 & 事实源 | `docs/release-readiness.md`；修正相互矛盾的文档 | `…` (prior) |
| B | 本地数据可靠 & 可恢复 | 冲突三方解决、`list()` 孤儿草稿自动注册、原子保存临时文件清理、DB 打开完整性检查 + 安全降级 | `087790a` 等 |
| C | 可观测 / 诊断 | 脱敏诊断快照命令、带轮转的结构化事件日志 + 禁记清单、诊断 UI（复制/导出）+ i18n | `f682139`/`338d674`/`6b81eb3` |
| D | E2E 验证工程 | BLOCKED 感知黄金路径、`dataDir` 隔离 fixture、Windows 手动脚本 | `145d743` |
| E | 发布就绪 | 发布 Runbook、manifest 一致性守护测试、fail-closed 构建已确认 | `529ee86` |
| F | 终验 | 全门禁、i18n 覆盖 & 安全审计、本交接文档 | 本提交 |

---

## 二、门禁状态（提交前全绿）

**前端**（`bun run test` / `bunx tsc --noEmit` / `bun run lint`）
- 测试：**127 passed**（13 文件），含 `src/diagnostics/api.test.ts` 的 5 个格式化器单测。
- `tsc --noEmit`：0 错误。
- `eslint`：0 error / 0 warning。

**Rust**（`cargo test --workspace` / `cargo clippy -- -D warnings` / `cargo fmt --check` / `cargo build`）
- 测试：**123 passed**（core 57 单元 + 66 集成含 `secrets`/`repository`/`schema`/`sources`/`settings`/`index_search_contract`；desktop 10 个 `updater_tests`）。
- `clippy -- -D warnings`：通过。
- `fmt --check`：通过。
- `cargo build`：成功（dev profile）。

> 注意：release 构建（`cargo tauri build` / `tauri build`）**未在本机执行**——它需要真实代码签名证书与推送端点（见 Phase E / Runbook），按约束不应由本次任务伪造。

---

## 三、i18n 覆盖

- 所有用户可见文案均经 `react-i18next` 的 `t()`，字符串存于 `src/i18n/locales/{zh-CN,en-US}.json`，无硬编码可见文案。
- **键位对等校验**：`zh-CN` 与 `en-US` 各 **187 个叶子键，零差异**（脚本核对）。
- 诊断模块新增 `diagnostics.*` 命名空间（导航、标题、介绍、快照/事件分区、刷新/复制/导出、已复制、加载错误、空日志），双语齐全。

---

## 四、安全审计（发布前置红线）

**原则**：诊断产物（快照 / 事件日志）是用户会复制外发的，必须脱敏；应用内错误提示仅对本人可见，不在审计范围。

1. **快照 `DiagnosticSnapshot`** 仅含：版本、生成时间、平台、SQLite 版本、DB 聚合（journal 模式、已应用迁移、完整性结论、各表行数）。**不含** API Key、草稿/来源正文、文件路径、源文件内容。
2. **事件日志 `EventLog`**：
   - 环形缓冲，写满丢最旧；结构化 `LogEntry { ts, level, name, fields }`。
   - `MUST_NOT_LOG_KEYS`（apiKey/token/secret/authorization/content/body/key/cookie/credential/password…）命中即静默替换为 `[redacted]`。
   - 来源索引事件经 `log_source_event` 仅记录 `source`（来源 id）+ 计数，**绝不记录 `rel_path` / `message`**。
3. **静态扫描**：全仓源码对 `sk-…`、私钥头、AWS Key、明文 password 等模式扫描，**无真实密钥**。唯一命中的是 `diagnostics.rs` 的脱敏*测试固件*（断言不泄露），非真实凭证。
4. **提交物**：扫描确认无密钥入仓。仓内 `tauri.conf.json` 的 `pubkey` 是**真实 minisign 公钥**
   （公钥是公开数据，进仓是设计如此），与之配对的**私钥不在仓内、不在任何文档里**，
   目前只在构建机上（见 `docs/RELEASE-RUNBOOK.md` §0 C3 的待办）。守护测试
   `shipped_updater_pubkey_is_a_real_minisign_key` 钉住 `dW50cnVzdGVkIGNvbW1lbnQ6` 前缀与长度——
   Tauri 官方测试回退串的前缀不同，被换回去就会红。
5. **AI Key**：存于系统凭据库，保存后不回显（见 `settings` i18n）。

---

## 五、发布就绪（Phase E 结论）

- 发布动作已迁到 CI：`v*` tag 触发 `.github/workflows/release.yml`（Windows 打包 → minisign 签名
  → GitHub Release 草稿 + 自动生成的 `latest.json`）。
- `src-tauri/build.rs` 的 fail-closed 只剩一条硬要求：release 构建必须有真实
  `INSIGHTFORGE_UPDATER_ENDPOINT`（空值与 dev 占位值都算缺，直接 panic）。
  `INSIGHTFORGE_CERT_THUMBPRINT` 缺省时只发 `cargo:warning`——安装包不签名，但 updater 校验照常。
- `tauri.conf.json`：`endpoints` 保留 dev 占位（release 由 build.rs 覆盖），
  `pubkey` 是真实 minisign 公钥，**不含任何 `${env.VAR}`**——该语法只对少数字段插值，
  其余会原样进二进制（曾导致 `signtool` 把占位串当证书名）。
- **4 个配置守护测试**：`committed_endpoint_is_dev_placeholder`、
  `shipped_updater_pubkey_is_a_real_minisign_key`、`config_does_not_second_guess_the_version`、
  `config_has_no_uninterpolated_env_placeholders`。

### 外部依赖（C1–C4，编号与 `docs/RELEASE-RUNBOOK.md` §0 一致）
- **C1** 代码签名证书（可选；缺省只是 SmartScreen 提示）。
- **C2** updater 清单端点：release.yml 已给出 GitHub Release 地址，但要求仓库真的被推送。
- **C3** updater minisign 私钥：密钥对已生成，**私钥尚未进 CI secret**（当前在构建机 temp 目录）。
- **C4** 安装 → 升级 E2E：未执行。

---

## 六、风险清单（P0 / P1 / P2）

### P0（发布前必清，缺一则不应发布）
- **R1 updater 私钥未托管**：密钥对已经生成，但私钥目前**只在构建机的 temp 目录**，
  没有 GitHub 仓库 secret。缺它时 release.yml 第一步就退出（预期的 fail-closed，不是 bug）；
  更糟的情形是私钥随 temp 丢失——已发布的客户端将永远收不到后续更新。见 `docs/RELEASE-RUNBOOK.md` §0。
- **R2 未在目标机（Windows 11，无 WebView2 的干净环境）跑过 E2E**：`e2e/golden-path.spec.ts` 因 `INSIGHTFORGE_E2E_RUN=1` 且 WebView2 可用才运行，当前环境无 WebView2，**7 条黄金路径全部 BLOCKED 而非假绿**。功能正确性由单元/集成测试与手动脚本 `e2e/manual/verify.ps1` 覆盖，但端到端未被本机证实。

### P1（发布后近期应处理）
- **R3 诊断版本号取自 Tauri 专有 API**：版本号本身已收敛到单一真源（workspace `Cargo.toml`，
  `config_does_not_second_guess_the_version` + CI 的 tag/Cargo/package.json 一致性检查钉住），
  原来的「两处版本漂移」风险已关闭。留下的是取用方式：
  `src/diagnostics/DiagnosticsView.tsx` 直接 `import { getVersion } from '@tauri-apps/api/app'`，
  绕过了 `CommandClient` seam——浏览器 runtime 里这一调用必然失败。归入网页 runtime 待办。
- **R4 错误提示可能含本地路径**：`CommandError.message` 会透出底层错误字符串（如 DB/IO 路径），仅在应用内对本人可见，不会进入外发诊断；但若后续接入遥测须先过滤。
- **R5 冲突三方解决的用户引导**：`DraftsView` 已暴露 `data-el` 钩子与三种选择，但无「为什么冲突」的解释文案，首次遇到易困惑（属反代写定位：提示优先于替写，可接受）。

### P2（可选增强）
- **R6 E2E 在 CI 中常态化**：需 CI 提供 WebView2 + CDP；当前脚本仅本地/手动。
- **R7 诊断快照增加「最近 N 次来源同步概览（计数级）」**：当前事件日志已含，但快照未聚合，求助时多一步复制。
- **R8 i18n 增加缺失语言/复数与 RTL**：当前仅 zh-CN/en-US，足够 1.0。

---

## 七、如何收尾（在你侧，非本次执行）

1. 准备 C1–C4 物料，按 `docs/RELEASE-RUNBOOK.md` 设置环境变量。
2. `bun run tauri build`（release）确认 fail-closed 解除并产出带签名安装包。
3. 在干净 Windows 11 机（装 WebView2）跑 `e2e/manual/verify.ps1` + 置 `INSIGHTFORGE_E2E_RUN=1` 跑 Playwright 黄金路径。
4. 验证通过后再 push / 建 PR / 发布——本次**故意止步于干净本地分支**。

---

## 八、本分支 cleanliness

- 已提交：Phase A–F 全部功能提交，每功能独立 commit。
- 未跟踪（按约定排除，未纳入提交）：`docs/redesign/`（早期原型，与本任务无关）。
- 无密钥、无草稿/来源内容、无用户路径进入提交或诊断产物。
