# 05 — 三栏工作台与 Markdown 编辑器

**What to build:** 用户得到可调三栏的写作工作台：CodeMirror 6 编辑器居中、正文宽度下限 35%，侧栏承载证据架与检索（本票据先以占位呈现）。中文输入法在长文中流畅无损；Markdown round-trip 语料测试建立，保证文件不被编辑器悄悄改写。

**Blocked by:** 01 — 桌面工作区骨架与 SQLite 地基

**Status:** ready-for-agent

- [ ] 三栏布局可调，正文宽度下限 35%；编辑器实现藏在 `EditorAdapter` 适配层后
- [ ] Markdown round-trip 语料测试全绿：打开 → 不做任何修改 → 保存，文件逐字节一致
- [ ] Windows 中文输入法下长文连续输入无丢字 / 错位（M0 第 1 项验证折叠于此，手动验证并记录）
- [ ] 能提取光标 / 选区前后段落与标题大纲（供 09 号票据的出站包使用）
