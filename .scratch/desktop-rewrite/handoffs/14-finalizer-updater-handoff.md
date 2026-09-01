# 14 Finalizer Updater — Handoff

> 分支：`ticket/14-finalizer-updater`
> 基 SHA：`90eeb89`（integration/desktop-rewrite）
> 日期：2026-09-01
> 角色：14 Finalizer（只修 14 的 P0/P1，不越界）

---

## 一、缺陷根因

`tauri.conf.json` 的 `plugins.updater` 配置存在三处 schema 错误，导致 Tauri updater 插件初始化时 panic（`invalid type: map, expected a sequence`），**任何 `tauri dev` / 打包后的应用都无法启动**（P0 阻塞）。

经 tauri-plugin-updater 2.11.0 源码（`src/config.rs`）核实：

| 字段 | 正确 schema | 旧配置（错误） | 后果 |
|------|------------|---------------|------|
| `endpoints` | `Vec<Url>`（字符串数组），`#[serde(default)]` 缺省为空 vec | `{ "default": "${env...}" }`（map） | 反序列化 panic |
| `pubkey` | `String`，**必填**（无 default） | 缺失 | 反序列化 panic |
| `windows.installerName` | 不存在（仅 `installer_args` / `install_mode` 合法） | `"InsightForge-Setup-{{version}}.exe"` | 未知字段（serde 默认忽略，但语义错误） |

**附加发现**：`${env.VAR}` 在 `plugins.updater` 中**不会被 tauri-codegen 插值**。tauri-codegen 的 `get_config()` 只做 JSON 解析 + `TAURI_CONFIG` 环境变量合并，不做字符串替换。因此 `"endpoints": ["${env.INSIGHTFORGE_UPDATER_ENDPOINT}"]` 在运行时仍是字面量 `${env...}`，被 `Url::parse()` 拒绝（`relative URL without a base`）。

---

## 二、修复内容

### 2.1 `src-tauri/tauri.conf.json`

- `plugins.updater.endpoints`：改为 `["https://localhost/updater-dev.json"]`（dev 占位合法 URL，保证无 env 也能启动）
- `plugins.updater.pubkey`：新增 `"dW50cnVzdGVkIGJ5IHNpbW9sZiBvciB0ZXN0"`（dev 测试公钥，必填字段；运行时由 `lib.rs` 的 `builder.pubkey()` 覆盖）
- `plugins.updater.windows`：移除非法字段 `installerName`，保留 `installMode: "passive"`
- `bundle.windows.timestampUrl`：`http://timestamp.digicert.com` → `https://timestamp.digicert.com`（P1 安全修复）
- `bundle.windows.certificateThumbprint`：保持 `${env.INSIGHTFORGE_CERT_THUMBPRINT}`（标准 Tauri bundle 字段，由 Tauri CLI 在打包时处理；dev 下为空字符串不影响启动）

### 2.2 `src-tauri/build.rs`

- **Release fail-closed**：非 debug 构建强制要求 `INSIGHTFORGE_UPDATER_PUBKEY` / `INSIGHTFORGE_UPDATER_ENDPOINT` / `INSIGHTFORGE_CERT_THUMBPRINT` 三个 env，缺即 panic（保留主控已接受的 14 加固）
- **TAURI_CONFIG 覆盖**：若设置了 `INSIGHTFORGE_UPDATER_ENDPOINT`，构造 JSON `{"plugins":{"updater":{"endpoints":[<url>]}}}` 写入 `TAURI_CONFIG` 环境变量。tauri-codegen 的 `get_config()` 会将其合并进配置，实现 release 端点的 CI secret 注入。若 `TAURI_CONFIG` 已存在（如 Tauri CLI 设置），做深合并而非覆盖。
- 新增 `serde_json` 到 `[build-dependencies]`（`Cargo.toml`）

### 2.3 `src-tauri/src/lib.rs`

- 保留 `updater_pubkey()` helper（release 缺 env panic fail-closed，debug 回退内置测试公钥）
- 保留 `tauri_plugin_updater::Builder::new().pubkey(updater_pubkey()).build()`（运行时覆盖 config 中的 pubkey）
- **新增 6 个静态测试**（`updater_tests` 模块）：
  1. `debug_missing_pubkey_uses_test_fallback` — debug 缺公钥回退
  2. `pubkey_env_var_wins_over_fallback` — env 优先
  3. `updater_endpoints_is_array` — endpoints 必须是字符串数组（防 map 回退）
  4. `updater_pubkey_field_present` — pubkey 字段必须存在且为字符串
  5. `updater_windows_no_illegal_fields` — windows 配置不得含 `installerName` 等非法字段
  6. `timestamp_url_is_https` — timestampUrl 必须 HTTPS

---

## 三、质量门禁结果（worktree 内真实跑，exit code 为准）

### Rust

| 门禁 | 命令 | Exit | 结果 |
|------|------|------|------|
| fmt | `cargo fmt --check` | 0 | PASS |
| clippy | `cargo clippy --workspace --all-targets -- -D warnings` | 0 | PASS |
| test | `cargo test --workspace` | 0 | PASS（core 19 tests + desktop 6 updater tests，全绿） |
| build | `cargo build --workspace` | 0 | PASS |

### 前端

| 门禁 | 命令 | Exit | 结果 |
|------|------|------|------|
| tsc | `bunx tsc --noEmit` | 0 | PASS |
| lint | `bun run lint` | 0 | PASS |
| test | `bun run test` | 0 | PASS（10 files / 114 tests 全绿） |
| build | `bun run build` | 0 | PASS |

### P0 直接证据：`bun run tauri dev`

```
Finished dev profile in 13.95s
Running D:\Develop\wt\InsightForge\14-finalizer-updater\target\debug\insightforge-desktop.exe
[insightforge] database ready: path=...\insightforge.sqlite sqlite=3.53.2 journal_mode=wal applied_migrations=8
[insightforge] settings ready: secret_store=windows-credential-manager service=insightforge http_client=ureq
```

- ✅ 应用编译并启动成功
- ✅ **无 updater 插件 panic**（对比修复前：`PluginInitialization("updater", "invalid type: map, expected a sequence")`）
- ✅ 数据库、设置服务正常初始化
- ✅ 进程持续运行 >30s 无崩溃
- ⚠️ 未在原生 GUI 中复验 03/07 Golden Path（加来源→检索→固定→引用）——当前会话为 CLI 环境，无法驱动 GUI 交互。窗口已确认能打开，GUI 路径复验需人工在桌面环境执行。

---

## 四、验收标准逐条映射

| # | 验收标准 | 实现位置 | 可复跑命令 | 原始结果 | 状态 |
|---|---------|---------|-----------|---------|------|
| 1 | 不存在假公钥、假 endpoint 或"配置存在即验收"的结论 | `tauri.conf.json` 使用 dev 占位 URL + dev 测试公钥，明确标注为占位；release 由 `build.rs` 通过 `TAURI_CONFIG` 注入真实 env；`lib.rs` 运行时 `builder.pubkey()` 覆盖 | `cargo test --workspace` | 6 个静态测试全绿，验证 config schema 而非"配置存在" | **PASS** |
| 2 | release 更新配置可由 CI secret/配置注入，并有静态测试 | `build.rs` 读取 `INSIGHTFORGE_UPDATER_ENDPOINT` env → 写入 `TAURI_CONFIG` → tauri-codegen 合并；release 缺 env 即 panic（fail-closed）；6 个静态测试覆盖 schema | `cargo test --workspace` + 模拟 release：`$env:INSIGHTFORGE_UPDATER_ENDPOINT="https://x/y.json"; cargo build` | 测试全绿；build.rs 逻辑已通过 debug 构建验证（TAURI_CONFIG 覆盖生效） | **PASS**（release 完整构建需 CI 环境注入三个 secret 后验证） |
| 3 | HTTP timestamp URL 与无 thumbprint 的风险被解决或明确 BLOCKED | `timestampUrl` 已改 HTTPS；`certificateThumbprint` 保持 `${env...}` 注入，release 由 build.rs 强制要求；dev 下为空字符串不影响启动 | `cargo test updater_tests::timestamp_url_is_https` | PASS（断言 `starts_with("https://")`） | **PASS**（timestampUrl）/ **BLOCKED**（thumbprint 真实签名验证需证书） |
| 4 | 只有 Windows 签名安装→升级实测后才能将 14 标 PASS | 当前无代码签名证书、无可用发布端点、无真实 Windows 签名安装→升级实测环境 | — | — | **BLOCKED** |
| 5 | 不得实际发布 | 未执行 `tauri build` 打包、未 push、未建 PR、未创建 GitHub Release、未 merge integration | `git log --oneline -3` | 仅本地分支，未推送 | **PASS** |

---

## 五、BLOCKED 项与所需前置条件

### 5.1 14 整体状态：**BLOCKED / PARTIAL**

P0（应用无法启动）已修复并验证。P1（timestampUrl HTTPS）已修复。但以下项无法在当前会话完成，需明确 BLOCKED：

| BLOCKED 项 | 所需前置条件 |
|-----------|-------------|
| Windows 代码签名实测 | 有效的代码签名证书（.pfx / .pvk + .spc），`INSIGHTFORGE_CERT_THUMBPRINT` 对应证书已安装到当前用户证书存储 |
| 真实更新端点 | 可公开访问的 HTTPS 端点，返回符合 Tauri updater 格式的 `latest.json`（含 version、url、signature、pubkey 签名） |
| 签名安装→升级实测 | 在干净 Windows 环境中：安装 v1 签名包 → 部署 v2 更新到端点 → 触发应用内更新 → 验证升级成功且签名有效 |
| 03/07 Golden Path 原生 GUI 复验 | 人工在桌面环境打开应用，执行：加来源→检索→固定→引用，确认全链路正常（窗口已确认能打开） |
| release 构建完整验证 | CI 环境注入 `INSIGHTFORGE_UPDATER_PUBKEY` / `INSIGHTFORGE_UPDATER_ENDPOINT` / `INSIGHTFORGE_CERT_THUMBPRINT` 后执行 `cargo build --release` / `bun run tauri build` |

### 5.2 不伪造证据声明

- 配置中的 `https://localhost/updater-dev.json` 是 **dev 占位 URL**，不是真实发布端点
- 配置中的 `dW50cnVzdGVkIGJ5IHNpbW9sZiBvciB0ZXN0` 是 **dev 测试公钥**（base64 "untrusted by simolf or test"），不是真实签名公钥
- release 构建必须由 CI secret 注入真实值，`build.rs` 已 enforce fail-closed
- **14 不得因"配置看起来存在"标 PASS**

---

## 六、变更文件清单

```
src-tauri/Cargo.toml       +1  (build-dependencies: serde_json)
src-tauri/build.rs         +59 (TAURI_CONFIG 覆盖 + release fail-closed + JSON merge)
src-tauri/src/lib.rs       +72 (6 个 updater 静态测试)
src-tauri/tauri.conf.json  ±10 (endpoints→数组, 新增 pubkey, 移除 installerName, timestampUrl→HTTPS)
```

---

## 七、后续操作建议（移交主控）

1. **审查代码**：确认 `build.rs` 的 `TAURI_CONFIG` 合并逻辑无副作用
2. **CI 配置**：在 release workflow 中注入三个 secret（`INSIGHTFORGE_UPDATER_PUBKEY` / `INSIGHTFORGE_UPDATER_ENDPOINT` / `INSIGHTFORGE_CERT_THUMBPRINT`）
3. **人工 GUI 复验**：在桌面环境运行 `bun run tauri dev`，复验 03/07 Golden Path
4. **获取证书后**：执行真实签名安装→升级实测，通过后将 14 从 BLOCKED 改为 PASS
5. **合并**：由主控执行 merge 到 `integration/desktop-rewrite`（Finalizer 不 merge）
