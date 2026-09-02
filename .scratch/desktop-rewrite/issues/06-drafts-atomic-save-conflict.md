# 06 — 草稿打开、原子保存与冲突三选一

**What to build:** 用户授权一个草稿输出目录后，可新建或打开真实的 `.md` 文件写作；保存是原子替换，断电或崩溃不会损坏草稿；文件被外部编辑器改过时，保存会呈现 diff 并让用户三选一，绝不静默覆盖。

**Blocked by:** 05 — 三栏工作台与 Markdown 编辑器

**Status:** ready-for-agent

- [ ] 草稿目录单独授权为可写；新建 / 打开 `.md` 后，数据库只存元数据（路径标识、保存基线、写作意图、布局），正文永远在磁盘
- [ ] 保存流程：同目录临时文件写入 → flush → sync → 平台替换（Windows `ReplaceFileW`，Unix 同文件系统 rename）；替换成功后才更新保存基线
- [ ] 打开时记录保存基线（file identity、内容 hash、mtime、size）；每次保存前重读比对
- [ ] 外部修改后保存返回结构化 `Conflict`，呈现 diff 三选一：保留本地 / 采用外部 / 另存为
- [ ] 断电、进程崩溃、锁占用、杀毒软件干扰的恢复用例通过（M0 第 3 项验证折叠于此）

> 2026-09-01 C-13-Fix 闭环：`crates/core/src/drafts.rs::open()` 现通过 `find_draft(filename)` 复用同路径草稿的 id/created_at，重打开不再触发 `UNIQUE(draft.path)` 约束。新增 Rust 测试 `open_existing_draft_reuses_id_and_created_at` 覆盖 create → open → open 路径；`open_creates_new_draft_when_no_prior_record` 覆盖无前置记录的 open 路径。P1 缺陷解除。
