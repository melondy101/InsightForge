# Release Runbook — 织知 InsightForge 桌面版

> 适用范围：把分支 `codex/desktop-release-readiness` 推进到「可长期个人使用、可诊断、可恢复、
> 可复跑验证」的 Windows 桌面应用，并准备 1.0 发布就绪工程。
> 本文件只描述**发布工程准备**，不做任何发布动作。
>
> **不可逾越的红线（贯穿始终）**：绝不推送 / 开 PR / 发布 / 伪造签名 / 伪造升级。
> 产品定位（本地优先、反代写、AI 只给问题/骨架/证据）不可变。

---

## 0. 发布还缺的外部条件（必须由用户提供）

发布动作已经由 `.github/workflows/release.yml` 接管（打 `v*` tag → Windows 打包 →
minisign 签名 → 上传 GitHub Release 草稿 → 自动生成 `latest.json`）。剩下的缺口是**凭据与验收**：

| ID | 依赖 | 当前状态 | 缺失时的行为 |
|----|------|----------|--------------|
| C1 | Windows 代码签名证书（thumbprint） | 未提供；**已降级为可选** | 安装包不签名，首次下载有 SmartScreen 提示；updater 校验不受影响（`build.rs` 只 warn） |
| C2 | updater 清单托管 | 端点由 release.yml 注入 `https://github.com/<repo>/releases/latest/download/latest.json`；**前提是该仓库真的被推送并有 Release** | 本地 `tauri build` 缺 `INSIGHTFORGE_UPDATER_ENDPOINT` → `build.rs` panic；装了包的客户端收不到更新 |
| C3 | updater minisign 私钥 | 密钥对已生成，公钥已随 `tauri.conf.json` 进仓；**私钥目前只在 `C:\Users\20145\AppData\Local\Temp\insightforge-updater.key`** | 未配置 `TAURI_SIGNING_PRIVATE_KEY` → release.yml 第一步就 fail-closed 退出；产不出 `.sig` 的包会被客户端拒绝 |
| C4 | 安装 → 升级 E2E | 未执行 | 依赖 C3 上线与真实环境 |

**用户待办（明确交接）**：
1. **立刻**把私钥内容存入 GitHub 仓库 secret `TAURI_SIGNING_PRIVATE_KEY`，并在仓库之外再备份一份
   （temp 目录随时会没）。丢了它，已发布的客户端永远收不到后续更新。有口令则配
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`，无口令留空。
2. 需要消除 SmartScreen 提示时，采购证书并配 `INSIGHTFORGE_CERT_THUMBPRINT`（可选）。
3. 推送分支与 tag（本仓库当前所有 commit 仍在本地）。
4. 在真实 Windows 上跑「安装旧版 → 升级新版」E2E（见 `e2e/manual/verify.ps1`）。

---

## 1. 发布构建的环境变量与守护测试

`src-tauri/build.rs` 只把**必须为真**的事留在构建期强制：

| 变量 | 用途 | 未设置时 |
|------|------|----------|
| `INSIGHTFORGE_UPDATER_ENDPOINT` | 真实 updater 清单地址（C2） | release 构建 panic（空值与 dev 占位值都算缺）；dev 构建不受影响 |
| `TAURI_SIGNING_PRIVATE_KEY`（+ `_PASSWORD`） | 给 `.msi`/`.exe` 产 minisign 签名 | Tauri CLI 报「有公钥无私钥」，构建失败 |
| `INSIGHTFORGE_CERT_THUMBPRINT` | Authenticode 代码签名（C1） | 只发 `cargo:warning`，照常出包，包不签名 |

机制上有一条必须记住的约束：**`tauri.conf.json` 里的 `${env.VAR}` 不会对所有字段插值**，
未插值的字符串会原样进二进制——endpoint 会让 updater 插件反序列化 panic，
certificateThumbprint 会让 `signtool` 把占位串当证书名。因此 endpoint 与证书都由
`build.rs` 以 `TAURI_CONFIG` 合并注入（`set_var` + `cargo:rustc-env` 两份缺一不可），
配置文件里只保留 dev 占位端点与**真实**签名公钥。

四条守护测试钉住这些事实（`src-tauri/src/lib.rs`）：
`committed_endpoint_is_dev_placeholder`、`shipped_updater_pubkey_is_a_real_minisign_key`、
`config_does_not_second_guess_the_version`、`config_has_no_uninterpolated_env_placeholders`。

版本号只有一个来源：workspace `Cargo.toml` 的 `version`（`tauri.conf.json` 故意不写 version）。
release.yml 会拒绝 tag / `Cargo.toml` / `package.json` 三者不一致的发布。

---

## 2. 构建流程

### 2.1 开发模式（不被阻塞）

```bash
bun install
bun run tauri dev        # 前端 + Rust，用 committed 公钥与 dev 占位 endpoint，可正常启动
```

### 2.2 发布模式（默认走 CI）

```bash
# 1) 改版本：workspace Cargo.toml（并同步 package.json）
git commit -am "chore: release v0.1.1"
git tag v0.1.1 && git push --tags    # 触发 release.yml
```

CI 依次跑：私钥存在性检查 → tag/版本一致性 → 前端三绿 → `tauri-apps/tauri-action@v0`
（构建 + minisign 签名 + 上传 + 生成 `latest.json`，Release 以草稿形式开出，人工点发布）。

### 2.3 本地复现发布构建（可选，只为验证产物）

```bash
export INSIGHTFORGE_UPDATER_ENDPOINT="https://github.com/<owner>/<repo>/releases/latest/download/latest.json"
export TAURI_SIGNING_PRIVATE_KEY="<私钥全文>"   # 只从环境读，绝不写进任何文件
bun run tauri build
```

产物在 `target/release/bundle/{msi,nsis}/`，每个安装包旁都有同名 `.sig`——
`.sig` 与 `latest.json` 一起上传到 Release，客户端据此校验；**不要手工编造 `latest.json`**。

---

## 3. 发布前门禁（提交前三绿 + Rust 全绿）

```bash
# 前端
bun run test && bunx tsc --noEmit && bun run lint && bun run build

# Rust
cargo test && cargo build && cargo fmt --check && cargo clippy -- -D warnings
```

四项均须 0 错误。任一失败即视为发布阻塞，先修后发。

---

## 4. 验收（发布就绪 ≠ 已发布）

- 本地：全套门禁通过、诊断快照可复制/导出、冲突三选一可用、数据库损坏可安全恢复。
- 真实 Windows：跑 `e2e/manual/verify.ps1` 清单（来源索引→中文检索、证据→引用、冲突三选一、
  删除来源保留快照、导出、无 AI Key 降级、重启状态保持、诊断脱敏）。
- 拿到 C1–C4 后：按第 2 节构建 → 签名 → 托管 manifest → 安装 → 升级 E2E。

> 本分支停在「发布就绪工程完成、未发布」的干净状态；真正的签名、托管、安装、升级由用户在
> 具备外部依赖后进行。
