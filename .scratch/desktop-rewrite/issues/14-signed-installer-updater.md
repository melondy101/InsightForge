# 14 — 签名安装包与自动更新

**What to build:** 用户下载签名的 NSIS / MSI 安装包完成安装，之后应用内可收到并安装签名更新——安装与升级全链路闭环。这是 1.0.0 发布列车：M1–M4 内容全部就绪后打第一个对外版本（M0 第 5 项验证折叠于此）。

**Blocked by:** 13 — 公众号剪贴板导出（wechat-clipboard-html）

**Status:** ready-for-agent

- [ ] 签名 NSIS / MSI 安装包，系统 WebView2 + bootstrapper（不捆绑 fixed runtime）
- [ ] updater 走静态 JSON / GitHub Releases，签名校验强制且不可关闭；安装 → 升级闭环手动验证并记录
- [ ] 打包版验证：关闭应用即退出，无后台进程驻留
