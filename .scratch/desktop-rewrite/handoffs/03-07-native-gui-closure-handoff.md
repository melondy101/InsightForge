# 03/07 Native GUI Closure Handoff — 桌面重写集成

## 基本信息

- **角色**: 03/07 Native GUI Closure Builder（P1-1 排序修复 + P2-2 格式一致化 + 原生 GUI Golden Path 闭环）
- **分支**: `ticket/03-07-native-gui-closure`
- **最终 HEAD**: `b9ddec0f8e0faf6c2303bf108c50b28324c7d06f`
- **基线**: `integration/desktop-rewrite @ 08e74a6`
- **工作树**: `D:\Develop\wt\InsightForge\03-07-native-gui-closure`
- **提交数**: 3（1a6fb9d → cb5e920 → b9ddec0）

## 提交历史

| SHA | 说明 |
|-----|------|
| 1a6fb9d | fix(03-07): P1-1 references ordering — fixed-width ISO timestamp with %S, rowid DESC tiebreaker, deterministic time injection |
| cb5e920 | fix(03-07): P2-2 drafts timestamp format consistency — align with references.rs %Y-%m-%dT%H:%M:%S.%6fZ |
| b9ddec0 | docs(03-07): native GUI golden path evidence via WebView2 CDP — full user loop verified, 0 console errors, 3 pre-existing gaps documented |

## 质量门禁结果（全部真实运行，exit code 为准）

| 门禁 | 命令 | Exit Code | 结果 |
|------|------|-----------|------|
| 前端测试 | `bun run test` | 0 | PASS — 114 tests (10 files) |
| TypeScript | `bunx tsc --noEmit` | 0 | PASS — 0 错误 |
| ESLint | `bun run lint` | 0 | PASS — 0 error/warning |
| Rust 测试 | `cargo test --workspace` | 0 | PASS — 全过（含 references 8 个 + scan 19 个等） |
| Rust 构建 | `cargo build --workspace` | 0 | PASS |
| Rust 格式 | `cargo fmt --check` | 0 | PASS |
| Rust clippy | `cargo clippy --workspace --all-targets -- -D warnings` | 0 | PASS — 0 warnings |

## P1-1 修复详情（references.rs 排序缺陷）

### 根因（已数值模拟证实）

- `references.rs` 原 `now_iso()` 格式为 `%Y-%m-%dT%H:%M:%fZ`，**缺失 `%S`（秒）**，时间戳丢失秒精度。
- chrono 0.4.45 的 `%f` 实际是 9 位零填充纳秒（非变宽去尾零），但因缺失 `%S`，跨秒时间戳字典序反转：5.9s → `"...00:900000000Z"` 排在 6.1s → `"...00:100000000Z"` 之后。
- `list()` 查询 `ORDER BY created_at DESC` 无并列兜底键。
- 原测试靠 `std::thread::sleep(10ms)` 掩盖，高负载时暴露。

### 修复位置

1. `crates/core/src/references.rs` — `now_iso(now: DateTime<Utc>)` 改为可注入时间，格式 `%Y-%m-%dT%H:%M:%S.%6fZ`（补回 %S + 微秒定宽 6 位）
2. `crates/core/src/references.rs` — `create_with_time(input, now)` 新增，生产 `create()` 委托 `create_with_time(input, Utc::now())`
3. `crates/core/src/references.rs` — `list()` 查询改为 `ORDER BY created_at DESC, rowid DESC`
4. `crates/core/src/references.rs` — 排序测试改为确定性时间戳（5.9s vs 6.1s，跨秒边界），无 sleep

### 可复跑命令

```bash
# 单个排序测试
cargo test -p insightforge-core --lib references::tests::list_references_returns_created_snapshots_ordered_by_created_at_desc -- --exact

# 全部 references 测试
cargo test -p insightforge-core --lib references::tests
```

### 20 次循环结果

```
Run 1: PASS (0 failed)
Run 2: PASS (0 failed)
...
Run 20: PASS (0 failed)
Result: 20/20 PASS, 0 flaky
```

## P2-2 修复详情（drafts.rs 时间戳格式一致化）

### 修复位置

- `crates/core/src/drafts.rs` — `now_iso()` 格式从 `%Y-%m-%dT%H:%M:%fZ`（缺失 %S）改为 `%Y-%m-%dT%H:%M:%S.%6fZ`，与 references.rs 一致。

### 说明

- 纯一致化，不改行为。drafts 当前无 ORDER BY，不影响排序。
- drafts 无单元测试（0 tests 编译通过）。
- GUI 验证：新草稿时间戳显示 `2026-09-01T12:04:40.798809Z`（正确格式），与旧格式 `2026-09-01T09:51:445673900Z`（缺失秒）并存，直观证明修复生效。

## Golden Path 证据路径与结果

### 证据目录

`.scratch/desktop-rewrite/golden-path/native-gui-cdp/`

### 核心文件

- `E2E-RESULT.md` — 完整端到端结果报告
- `gp2-step1-indexed.png` — 来源添加并索引
- `gp2-step2-draft-created.png` — 草稿创建（正确时间戳格式）
- `gp2-step3-results.png` — 搜索结果（3 张证据卡）
- `gp2-step3-pinned.png` — 固定成功（"已固定"）
- `gp2-step4-references.png` — 引用视图（真实数据）
- `gp2-step5-source-deleted.png` — 删除来源后
- `gp2-step6-snapshot.png` — 快照保留验证
- `gp-final-sources.png` — reload 后确认 0 来源
- `console-gp2-full.jsonl` — 控制台日志（0 errors）

### 执行方法

- **CDP 连接**: `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222 --remote-allow-origins=*`
- **驱动**: Python `websocket-client` + `requests`，直接 CDP WebSocket
- **UI 交互**: `Runtime.evaluate` DOM `click()`（无合成鼠标）
- **对话框**: `window.confirm` override（返回 true）

### 验收结果

| 验收项 | 结果 |
|--------|------|
| 添加真实来源并索引 | ✅ PASS |
| 搜索返回真实证据卡 | ✅ PASS（3 张卡） |
| 打开/新建真实草稿 | ✅ PASS |
| 点击"固定到草稿" | ✅ PASS（按钮→"已固定"） |
| 引用视图显示真实引用 | ✅ PASS（来源/文件/行号/snippet/采用） |
| adopted/sourceDeleted 状态 | ⚠️ PARTIAL（adopt 按钮存在；sourceDeleted 未自动接线） |
| 删除来源（CDP DOM click） | ✅ PASS（reload 后确认 0 来源） |
| 快照保留 | ✅ PASS（删除后引用内容仍在） |
| 无 console error | ✅ PASS（0 errors） |
| 无 native panic | ✅ PASS |
| 无 i18n key 原文 | ✅ PASS（界面显示翻译文本） |

## 遗留缺口 / 预存问题（非本票引入）

1. **SourcesView 状态同步**: `remove` action 只更新 local `useSources` state，但 view 在 `appState.sources` 非空时优先用全局 state → 删除后 UI 不刷新，需 reload。
2. **sourceDeleted 未自动接线**: `sources_remove` 不调用 `references_mark_source_deleted` → 删除来源后引用不显示"来源已删除"标记。`mark_source_deleted` 方法存在且单测通过，只是未接入删除流程。
3. **drafts `open()` UNIQUE 约束**: `generate_id()` 每次生成新 ID（filename + 时间戳纳秒），但 `upsert_draft` 用 `ON CONFLICT(id)` → 打开已有草稿违反 `UNIQUE(draft.path)`。新建草稿正常，重新打开已有草稿失败。

以上 3 项均为预存问题，非本票 P1-1/P2-2 改动引入。建议后续票单独修复。

## 锁定文件确认（未修改）

- `src/search/SearchView.tsx` — 未修改（已正确接线，无 defect）
- `src/i18n/locales/zh-CN.json` — 未修改（172 key，无缺失）
- `src/i18n/locales/en-US.json` — 未修改（172 key，与 zh 完全对齐）
- `src-tauri/src/lib.rs` — 未修改
- `src-tauri/tauri.conf.json` — 未修改
- updater/签名/E2E 架构 — 未修改
- 06 原子保存、04 watcher、09 AI 功能 — 未修改

## 禁止项确认

- 未 push 到远端
- 未 merge 进 integration
- 未关 issue
- 未改 _integration 或其他 worktree
- 未用 mock 数据替代原生链路
- 未删测试/弱化断言/用 as any 掩盖
- 未伪造日志/截图/测试结果
