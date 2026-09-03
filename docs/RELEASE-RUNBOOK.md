# Release Runbook — 织知 InsightForge 桌面版

> 适用范围：把分支 `codex/desktop-release-readiness` 推进到「可长期个人使用、可诊断、可恢复、
> 可复跑验证」的 Windows 桌面应用，并准备 1.0 发布就绪工程。
> 本文件只描述**发布工程准备**，不做任何发布动作。
>
> **不可逾越的红线（贯穿始终）**：绝不推送 / 开 PR / 发布 / 伪造签名 / 伪造升级。
> 产品定位（本地优先、反代写、AI 只给问题/骨架/证据）不可变。

---

## 0. 不可发布的外部依赖（必须由用户提供）

以下四项（与 `release-readiness.md` 的 C1–C4 对应）**本分支未做、也无法在本地伪造**，
必须由用户在真实 Windows 环境提供后才能发布：

| ID | 依赖 | 当前状态 | 缺失时的行为 |
|----|------|----------|--------------|
| C1 | 代码签名证书（含 thumbprint） | 未提供 | release 构建失败（fail-closed） |
| C2 | HTTPS updater endpoint（`https://…/updater.json` 托管） | 未提供；`tauri.conf.json` 现为 dev 占位 `https://localhost/updater-dev.json` | 升级不可用（按红线不伪造升级） |
| C3 | updater 签名密钥对（公钥上架 manifest） | 未提供 | release 构建失败（fail-closed） |
| C4 | 安装 → 升级 E2E | 未执行 | 依赖 C1–C3 与真实环境 |

**用户待办（明确交接）**：
1. 申请/采购 Windows 代码签名证书，记录其 thumbprint。
2. 准备 HTTPS 托管的 `updater.json` 端点与对应私钥。
3. 用私钥签名 `updater.json`（与 C3 公钥配对）。
4. 在已签名安装包上跑「安装旧版 → 升级新版」E2E（见 `e2e/manual/verify.ps1`）。

---

## 1. 发布构建的环境变量（fail-closed）

release 构建（`cfg!(not(debug_assertions))`）由 `src-tauri/build.rs` 强制校验以下变量，
**缺失即 panic，绝不会静默产出不完整产物**：

| 变量 | 用途 | 未设置时 |
|------|------|----------|
| `INSIGHTFORGE_UPDATER_PUBKEY` | updater 公钥（与 C3 私钥配对） | release 启动 panic（`updater_pubkey()` fail-closed） |
| `INSIGHTFORGE_UPDATER_ENDPOINT` | 真实 HTTPS updater 端点（C2） | release 构建 panic；dev 回退到占位 URL |
| `INSIGHTFORGE_CERT_THUMBPRINT` | 代码签名证书指纹（C1） | `tauri.conf.json` 的 `${env.INSIGHTFORGE_CERT_THUMBPRINT}` 留空 → 签名步骤失败 |

> `updater.pubkey` 在 `tauri.conf.json` 中当前是**测试公钥**；开发模式（`debug`）回退到它，
> 保证本地 `tauri dev` 可运行；release 必须用 `INSIGHTFORGE_UPDATER_PUBKEY` 覆盖。

---

## 2. 构建流程

### 2.1 开发模式（不被阻塞）

```bash
bun install
bun run tauri dev        # 前端 + Rust，使用测试公钥与占位 endpoint，可正常启动
```

### 2.2 发布模式（必须齐全）

```bash
export INSIGHTFORGE_UPDATER_PUBKEY="<与 C3 私钥配对的 base64 公钥>"
export INSIGHTFORGE_UPDATER_ENDPOINT="https://<你的域名>/updater.json"
export INSIGHTFORGE_CERT_THUMBPRINT="<C1 证书 thumbprint>"

bun run tauri build      # 缺失任一变量 → build.rs panic，明确报错缺哪些
```

### 2.3 升级 manifest（外部，C2/C3）

把 `tauri build` 产出的 `target/release/bundle/.../*.msi`/`.exe` 版本信息，
用 C3 私钥签名生成 `updater.json` 并托管到 C2 的 HTTPS 端点。
**此步骤依赖外部密钥与基础设施，不在本仓库执行。**

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
