# Golden Path · Ticket 18 — 来源增量同步进度补验

## 环境
- OS：Windows 10.0.26200 x64
- Rust：1.90.0 (840b83a10)
- Tauri CLI：2.11.4
- Node/Bun：1.3.11
- 前端：Vite 8.2.2, React 19.2.8
- 集成基线：`integration/desktop-rewrite` HEAD = `7339d6f`

## 前置条件
```bash
cd D:/Develop/wt/InsightForge/18-sources-incremental-sync-golden-path
bun install
```

## 场景 1：首次索引
1. 运行 `bun run tauri dev`
2. 应用启动后，点击侧栏「来源管理」
3. 点击「选择目录…」，选择一个包含 2+ 个 Markdown/文本文件的空文件夹
4. 输入名称，点击「添加并索引」
5. **预期**：来源行出现，进度条从 0% 开始，标签显示 `索引中 0/N`
6. **预期**：进度单调递增，最终显示 `索引中 N/N`
7. **预期**：完成后进度条消失，状态变为「空闲」

## 场景 2：无变化同步
1. 在首次索引完成后，点击同一来源的「同步」按钮
2. **预期**：进度条重新出现，从 0/N 递增到 N/N
3. **验证**：打开 SQLite 数据库，确认 fragment 和 fragment_fts 数量未增加

## 场景 3：单文件变化同步
1. 修改来源中的任意一个文件（保存）
2. 点击「同步」按钮
3. **预期**：进度条显示 N/N（N 为总文件数）
4. **验证**：仅被修改的文件产生新的 fragment，其他文件不变

## 自动化证据（本 ticket 新增）

### Rust 集成测试（crates/core/tests/sources.rs）
- `sync_emits_progress_events_for_unchanged_files` — 验证无变化同步事件序列：BackfillStarted → FileIndexed×2 (processed=1,2; fragment_count=0) → BackfillCompleted
- `sync_reindexes_only_the_changed_file` — 验证单文件变化：仅变更文件 fragment_count>0，其他为 0
- `repeated_sync_never_duplicates_fragments` — 验证 3 轮重复 sync 后 fragment/FTS 恒为 (2, 3, 3)

### 前端单元测试（src/sources/use-sources.test.ts）
- `backfill_started 将进度初始化为 0/N` — 验证初始进度状态
- `file_indexed 事件序列使 processed 单调递增到 N` — 验证进度单调性
- `backfill_completed 后进度条目被清除` — 验证完成事件清理
- `无变化同步的 file_indexed 事件携带 fragment_count=0` — 验证无变化同步事件
- `处理多个来源的并发进度` — 验证多来源并发

### 前端纯函数测试（src/sources/progress.test.ts，既有）
- `progressPercent` 3 项测试验证百分比计算边界

## 当前环境限制
- 本环境为无头/远程 shell，无法进行 GUI 交互验证
- `tauri dev` 编译并启动成功（exit code 143 = timeout killed），证明桌面壳可运行
- 完整 GUI Golden Path 需在有显示的环境中手动执行上述步骤

## 运行命令与预期结果
```bash
# Rust 集成测试
cd src-tauri && cargo test --workspace
# 预期：19 个 sources 测试全通过，包括新增的增量同步测试

# 前端测试
bun run test
# 预期：9 个测试文件，106 个测试全通过（含新增的 use-sources.test.ts）

# 代码质量
bun run lint && bunx tsc --noEmit
# 预期：0 error, 0 warning
```
