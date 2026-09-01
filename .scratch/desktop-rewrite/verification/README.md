# Windows 验证执行指南

> 当前 WSL 环境缺少 `bun`/`cargo`/`rustc`，全部门禁与 GUI/E2E 验证必须在 **Windows 10/11** 或 **CI** 环境执行。

## 一、前置条件（Windows 环境）

1. **Git for Windows**（提供 Git Bash）
2. **Bun**：`powershell -c "irm bun.sh/install.ps1 | iex"`
3. **Rust**：`rustup default stable`（需 1.90+）
4. **Playwright**：`bun run test:e2e:install`
5. **WebView2**：系统自带或单独安装
6. **签名证书**（仅 14 需要）：`INSIGHTFORGE_CERT_THUMBPRINT`

## 二、验证步骤

### 步骤 1：克隆 worktree（如尚未有）
```bash
git worktree add D:/Develop/wt/InsightForge/_integration -b integration/desktop-rewrite integration/desktop-rewrite
cd D:/Develop/wt/InsightForge/_integration
```

### 步骤 2：运行门禁脚本
```bash
cd /mnt/d/Develop/InsightForge/.scratch/desktop-rewrite/verification
bash windows-gate-and-gp.sh
```

脚本会将全部输出追加到 `verification/evidence-YYYYMMDD-HHMMSS.txt`。

### 步骤 3：手动 GUI Golden Path（03/07）
1. `cd D:/Develop/wt/InsightForge/_integration`
2. `bun run tauri dev`
3. 按脚本内 `MANUAL` 节操作，截图保存到 `verification/`

### 步骤 4：签名安装验证（14）
1. 配置环境变量：
   ```bash
   export INSIGHTFORGE_UPDATER_PUBKEY="<真实公钥>"
   export INSIGHTFORGE_UPDATER_ENDPOINT="https://updater.insightforge.app/{{target}}/{{current_version}}"
   export INSIGHTFORGE_CERT_THUMBPRINT="<证书指纹>"
   ```
2. `bun run tauri build`
3. 签名、安装、触发更新，截图记录

### 步骤 5：E2E 验证（15）
```bash
cd D:/Develop/wt/InsightForge/_integration
export INSIGHTFORGE_CDP_ENDPOINT="http://127.0.0.1:9222"  # 若 WebView2 启用 CDP
bun run test:e2e:headed
```

## 三、结果回传

将以下文件打包回传：
- `verification/evidence-*.txt`
- `verification/gp-03-07-*.png`
- `verification/gp-14-*.png`
- `e2e-report/`（若 15 E2E 执行）

## 四、当前 BLOCKED 清单

| 项目 | 阻塞原因 | 解除条件 |
|------|----------|----------|
| 03/07 GUI 验证 | 无 Windows 桌面环境 | 提供 Windows 机器或 VM |
| 14 签名安装→升级 | 无签名证书 + 发布端点 | 配置签名证书 + GitHub Releases / static JSON |
| 15 E2E | 无 WebView2 CDP 自动化连接 | 启用 WebView2 CDP 或验证 `@tauri-apps/playwright` |
| 全部门禁 | WSL 无 bun/cargo/rustc | 在 Windows/CI 环境执行脚本 |
