# 12 — 通用 HTML 导出（generic-html）

**What to build:** 用户把草稿一键导出为语义化、已消毒、含 Clean Reading 阅读排版的完整 HTML 文件，可直接粘贴或导入 WordPress 等 CMS；图片不可访问时导出被阻断并明确提示，不产生表面成功实际丢图的内容。导出过程不反向修改 Markdown 原稿。

**Blocked by:** 06 — 草稿打开、原子保存与冲突三选一

**Status:** ready-for-agent

- [ ] 导出层在 TypeScript 侧：remark-parse → remark-gfm → remark-rehype → rehype-stringify（输出 HTML 必须加 sanitize），vitest 覆盖
- [ ] 一个固定 Clean Reading 主题 + 一个强调色：16px 正文、1.8 行距、层级清晰的标题、浅色引用块、系统字体栈、移动端安全的图片与表格
- [ ] 图片仅允许可公开访问的远程 `https` URL；不可访问阻断导出并明确提示
- [ ] Markdown 内嵌 HTML 默认禁用；不安全协议链接降级为纯文本
- [ ] 导出为派生物：原稿逐字节不变
