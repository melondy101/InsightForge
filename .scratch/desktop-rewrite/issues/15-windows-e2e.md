# 15 — Windows 全链路 E2E

**What to build:** 用 WebDriver 把真实用户路径完整串起来：目录选择 → 索引进度 → 检索证据卡 → 写作保存 → 外部冲突 → AI consent → 导出。这是 M4 的验收线：E2E 在打包版（而非仅开发模式）上通过，1.0.0 才算就绪。

**Blocked by:** 13 — 公众号剪贴板导出（wechat-clipboard-html）

**Status:** ready-for-agent

- [ ] WebDriver E2E 覆盖：目录选择、索引进度、证据卡、编辑保存、外部冲突、AI consent、导出
- [ ] 全链路在签名打包版上通过（安装产物验证，而非仅开发模式）
- [ ] 纯浏览器 renderer 模式不作为替代（必须真实文件与安装验证）
