# 17 — 修复增量同步真实进度

**What to build:** 修复 02 遗留的增量同步进度错误：未变文件快速跳过时不递增 `processed`、不发送进度事件。首次和增量同步都应显示从 0 到总文件数的真实扫描进度；未变文件不重解析，但仍计入扫描进度。

**Status:** merged — merge commit `8e5f892` into `integration/desktop-rewrite`

**Blocked by:** 02 — 添加来源并完成首次补扫索引

## 验收标准

- 增量同步含未变文件时，进度单调递增并最终等于 total；
- 未变文件不产生重复 fragment/FTS 写入；
- 完成事件与 UI 状态正确收敛；
- 真实临时目录 + SQLite 集成测试覆盖；
- Tauri/浏览器 Golden Path 覆盖"首次索引后再次同步"。

## 允许范围

- `crates/core/src/indexer.rs`
- 必要的 `sources` 进度模型
- 相关 Rust 集成测试
- 来源 UI 投影测试
- `sources.*` i18n

## 禁止范围

- 不实现 04 的 watcher、重命名、定时校验或重建索引。

## 推荐验证

- Rust sources/indexer 集成测试；
- 前端 hook/视图行为测试；
- Tauri Golden Path。
