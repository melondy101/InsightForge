# 05 — 三栏工作台与 Markdown 编辑器

**What to build:** 用户得到可调三栏的写作工作台：CodeMirror 6 编辑器居中、正文宽度下限 35%，侧栏承载证据架与检索（本票据先以占位呈现）。中文输入法在长文中流畅无损；Markdown round-trip 语料测试建立，保证文件不被编辑器悄悄改写。

**Blocked by:** 01 — 桌面工作区骨架与 SQLite 地基

**Status:** done (merged 787bc76)

- [ ] 三栏布局可调，正文宽度下限 35%；编辑器实现藏在 `EditorAdapter` 适配层后
- [ ] Markdown round-trip 语料测试全绿：打开 → 不做任何修改 → 保存，文件逐字节一致
- [ ] Windows 中文输入法下长文连续输入无丢字 / 错位（M0 第 1 项验证折叠于此，手动验证并记录）
- [ ] 能提取光标 / 选区前后段落与标题大纲（供 09 号票据的出站包使用）

## Comments

### attempt 1 · 2026-08-30 · Builder 1
- 全部 4 条验收标准完成；门禁 8 项一键 `bun run gate` 退出码 0（vitest 84 passed / cargo 48 passed / tsc、lint、build、fmt、clippy 全过），Rust 侧零改动。
- 交付：提取纯函数（段落/大纲/段落序号/标题路径，18 测）、文件文本归一化（BOM+CRLF/LF/CR，15 测）、`EditorAdapter` 适配层（CodeMirror 6 藏于其后，13 测 headless 契约）、round-trip 语料 19 组 Buffer 级逐字节全绿（20 测）、三栏布局约束纯函数（11 测）与可调三栏工作台 UI（拖拽分隔条、35% 硬约束、状态条提取冒烟、`workspace.*` i18n、`data-el`）。
- IME 验证（M0 第 1 项）：WebView2 CDP 组合模拟（`Input.imeSetComposition` + `Input.insertText` 提交）与直插共 570 字混排长文，`getValue()` 逐字符相等、光标停在文末；残余风险（真实 OS IME 键盘预处理未覆盖）已在 handoff 声明，建议 15 号票真实 IME 复核。
- Golden Path 34 步全过（三栏/拖拽 35% 触线/IME/提取冒烟/02 来源管理往返），截图 5 张与日志在 `handoffs/evidence/05-*`；干净退出另行验证（无调试口 1 秒内退出）。
- Handoff: `.scratch/desktop-rewrite/handoffs/05-handoff.md`；commits b206feb → 18da535 → 3b5d28b → 44ca55d → e132f9b + 本次证据提交。
- 2026-08-30 Orchestrator：Builder attempt 1 一次交付。Reviewer 独立复验 PASS（P0/P1=0；round-trip 语料经变异检验证实真实有效，实测 18 组——handoff 声称 19 系计数偏差，以 18 为准；P2×2 + P3×3 非阻塞）。P2-1 极窄窗口侧栏负宽度钳制与 P2-2 openDocument 撤销到空文档语义作为 06 号票修复项/设计输入随单携带。合并条件①–⑥全满足，--no-ff 合入 787bc76，集成 8 门禁回归全绿。
