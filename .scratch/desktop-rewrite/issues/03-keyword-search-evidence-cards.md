# 03 — 中文关键词检索与证据卡

**What to build:** 用户输入中文关键词，即得到按相关度排序的证据卡：片段文本、来源标题（不含本机路径）与片段上下文；两字词也能出结果。10k 文件规模下检索流畅（M0 第 2 项验证折叠于此）。

**Blocked by:** 02 — 添加来源并完成首次补扫索引

**Status:** done

- [x] FTS5 `bm25()` 排序，`highlight()` / `snippet()` 生成片段上下文
- [x] 中文 trigram tokenizer 实测达标（质量与索引体积记录在案）
- [x] 两字符查询回退受限线性扫描，行为有测试覆盖
- [x] 证据卡呈现片段文本、来源标题、相关度；10k 文件基准下首次索引、增量更新与检索流畅

> 2026-09-01 C-03-07 票验证：原生 Tauri GUI Golden Path 通过 WebView2 CDP 驱动，搜索"知识管理"返回 3 张证据卡（片段/来源/相关度/固定按钮），固定到草稿成功。0 console errors，0 i18n key 泄漏。
>
> 2026-09-01 C-13-Fix 追加：`crates/core/src/search.rs::fallback_linear_scan` 重写为字符级迭代（消除多字节 UTF-8 字节切片 panic），新增 5 个 Rust 测试覆盖「二字词命中、空结果、边界匹配、limit 裁剪、FTS5 主路径」。验证标准 #6「二字词回退行为有可运行测试」由 C-13-Fix 闭环。
