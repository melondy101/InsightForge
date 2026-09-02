# InsightForge Windows E2E

`e2e/golden-path.spec.ts` 是 Windows 桌面验收的待完善规格，不能替代真实 Tauri/WebView2 验证，也不应把安装包当作 HTTP server。

## 当前基线

- 已在真实 Tauri dev 实例通过 WebView2 CDP 验证核心来源、草稿、搜索和引用调用链。
- Playwright 规格仍含占位/跳过项；在这些断言改为真实交互前，不得宣称 Windows 安装包 E2E 通过。
- `@tauri-apps/playwright` 不在依赖树中；不要添加它，也不要在 `playwright.config.ts` 配置 `webServer` 来启动 `.exe`。

## 真实验收前置条件

1. Windows 10/11 与 WebView2 Runtime。
2. 真实签名安装包、可访问的 HTTPS updater endpoint，以及安全注入的 updater 私钥（仅对公开发布需要）。
3. 启动时明确启用 WebView2 CDP 端口，并通过 `INSIGHTFORGE_CDP_ENDPOINT` 连接。

```powershell
$env:INSIGHTFORGE_CDP_ENDPOINT = 'http://127.0.0.1:9222'
bun run test:e2e
```

个人自用不要求购买代码签名证书；但这不构成公开发布签名或安装升级验证通过的证据。
