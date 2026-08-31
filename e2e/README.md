# InsightForge Windows E2E

## 环境要求

- Windows 10/11
- WebView2 Runtime（系统已自带或单独安装）
- 签名安装包（`InsightForge-Setup-<version>.exe`）
- Playwright 浏览器：`bun run test:e2e:install`

## 自动化路径

Tauri 安装包不是 HTTP server，Playwright 的 `webServer` 配置不适用。

### 方案 A：WebView2 CDP（推荐）

1. 启动 InsightForge 安装包
2. 通过 WebView2 CDP 端口连接（需在启动时启用）
3. 使用 Playwright `chromium` 类型驱动

```bash
# 设置 CDP 端点（需 WebView2 启用开发者功能）
export INSIGHTFORGE_CDP_ENDPOINT="http://127.0.0.1:9222"
bun run test:e2e
```

### 方案 B：@tauri-apps/playwright（待验证）

`@tauri-apps/playwright` 已安装，但 Tauri 2 的 WebView2 自动化 adapter
需确认其是否支持直接连接已安装的桌面应用。

## 当前状态

- E2E 测试框架已搭建（Playwright + 选择器）
- 选择器未在真实 WebView2 渲染中验证
- 完整 7 路径 Golden Path 待 Windows 环境执行

## BLOCKED 条件

- 无 Windows 签名安装包
- 无 WebView2 CDP 自动化连接验证
- 无签名证书与发布基础设施
