# 13 — 公众号剪贴板导出（wechat-clipboard-html）

**What to build:** 用户一键复制公众号格式的 HTML（标签 / 属性白名单、样式内联），粘贴进微信编辑器后排版保持；同时写入 `text/html` 与 `text/plain` 剪贴板。与 12 号票据共享同一 Markdown AST 与主题。

**Blocked by:** 12 — 通用 HTML 导出（generic-html）

**Status:** ready-for-agent

- [ ] 同一 Markdown AST 生成标签 / 属性受限的片段；主题 CSS 安全部分计算并内联（借鉴 doocs/md + Juice 模式，不引入完整渲染器依赖）
- [ ] 同时写入 `text/html` 与 `text/plain` 两种剪贴板格式
- [ ] 手动粘贴微信编辑器验证排版保持，验证步骤记录在案
- [ ] 不引入微信 API、图片托管或账号密钥
