# 03/07 Finalizer Handoff — 桌面重写集成

## 基本信息

- **角色**: 03/07 Finalizer（只修 Reviewer 已列出的 P0/P1）
- **分支**: `ticket/03-07-finalizer`
- **最终 HEAD**: `954f9e1631d7dfef7311ab085235ae13066b169f`
- **基线**: `integration/desktop-rewrite @ 1c764d3`
- **工作树**: `D:\Develop\wt\InsightForge\03-07-finalizer`
- **提交数**: 3（02dca54 → bb2080a → 954f9e1）

## 提交历史

| SHA | 说明 |
|-----|------|
| 02dca54 | fix(03-07): resolve P0/P1 defects — shared state, type errors, search-to-reference wiring |
| bb2080a | test(03-07): add shared state provider tests and Rust references persistence tests + backend bug fix |
| 954f9e1 | style(03-07): cargo fmt formatting |

## 质量门禁结果（全部真实运行，exit code 为准）

| 门禁 | 命令 | Exit Code | 结果 |
|------|------|-----------|------|
| 依赖安装 | `bun install --frozen-lockfile` | 0 | PASS — 341 packages |
| TypeScript | `bunx tsc --noEmit` | 0 | PASS — 0 错误 |
| ESLint | `bun run lint` | 0 | PASS — 0 error/warning |
| 前端测试 | `bun run test` | 0 | PASS — 114 tests (10 文件) |
| 前端构建 | `bun run build` | 0 | PASS — Vite build 成功 |
| Rust 格式 | `cargo fmt --check` | 0 | PASS |
| Rust clippy | `cargo clippy --workspace --all-targets -- -D warnings` | 0 | PASS — 0 warnings |
| Rust 测试 | `cargo test --workspace` | 0 | PASS — 92 tests 全过 |
| Rust 构建 | `cargo build --workspace` | 0 | PASS |

## 验收标准逐项映射

### 1. App 与 DraftsView 不再使用彼此独立、不同步的 hook state；单一共享状态源

**结果: PASS**

**实现位置**:
- `src/AppStateContext.tsx`: 新增 `DraftsModelContext` + `useDraftsModel()` hook
- `src/App.tsx`: App 调用一次 `useDrafts()`，经 `DraftsModelContext.Provider` 下发；`AppStateContext` 的 `activeDraft` 来自 `draftsModel.activeDraft`；`AppActionsContext` 三个动作委托给真实的 `sourceActions` / `draftsModel.actions`（不再是 no-op）
- `src/drafts/DraftsView.tsx`: 改用 `useDraftsModel()` 从 context 消费，不再自行调用 `useDrafts()`

**可复跑命令**: `bun run test`（AppStateContext.test.tsx 验证 provider/consumer  round-trip）

### 2. 不存在 __demo__ 或伪造 ID；demo fixture/真实数据在类型与 UI 中清晰区分

**结果: PASS**

- 代码中无 `__demo__`、无伪造 ID
- 测试中使用的 fixture（`makeDraftFile`、`seed_fragment`）均在测试文件内部，明确标注为测试数据
- 前端无硬编码的 demo 数据注入生产 UI

### 3. 搜索 → 固定 → 引用 → 引用列表可见为真实用户路径

**结果: PASS（前端接线完成；原生 GUI 端到端验证因 tauri.conf.json 锁定文件的预存错误而 BLOCKED）**

**实现位置**:
- `src/search/SearchView.tsx`: 每个搜索结果卡片新增「固定到草稿」按钮（`data-el="search-card-pin"`），调用 `referencesApi.create(activeDraft.id, card.fragmentId)`；无 activeDraft 时按钮 disabled；固定成功后显示「已固定」
- `src/references/api.ts`: `referencesApi.create` 调用 `references_create` Tauri command（已有，原为死代码，现已被前端接线激活）
- `src/references/useReferences.ts`: `actions.create` 调用 API 并将新引用插入列表头部
- `src/references/ReferencesView.tsx`: 列出引用，显示来源/路径/行号/snippet，支持 adopt，显示 sourceDeleted/adopted badge

**后端修复**: `crates/core/src/references.rs` 中 `FragmentSnapshot.source_file_id` 原为 `String` 但 `source_file.id` 是 `INTEGER`，导致 `references_create` 运行时 `InvalidColumnType` panic（这是该 command 成为死代码的根因）。已修复为 `i64`。

**可复跑命令**: `cargo test references::tests`（8 个测试覆盖 create/list/adopt/mark_source_deleted）

### 4. 删除来源/重建索引后，不可变引用快照仍存在并标明来源不可用

**结果: PASS**

**实现位置**:
- 后端 `crates/core/src/references.rs`: `mark_source_deleted` 将 `source_deleted=1`，不删除引用行；`list` 返回 `source_deleted` 字段
- 前端 `src/references/ReferencesView.tsx`: `ref.sourceDeleted` 为 true 时显示「来源已删除」badge（`data-el="reference-source-deleted"`）
- Tauri command `references_mark_source_deleted` 已在 `src-tauri/src/lib.rs` 注册（原有）

**可复跑命令**: `cargo test references::tests::mark_source_deleted_preserves_snapshot_and_flags_source_unavailable`

### 5. 有浏览器 Golden Path（真实 GUI 驱动，不只单测）

**结果: PARTIAL PASS**

- **Vite dev server + 浏览器驱动**: PASS — 真实启动 `bun run dev`（localhost:1420），用浏览器自动化验证了应用渲染、侧栏导航、4 个视图切换、控制台 0 error/warning
- **原生 Tauri GUI（bun run tauri dev）**: BLOCKED — 因 `src-tauri/tauri.conf.json`（锁定文件）中 `plugins.updater` 配置类型错误（map 而非 sequence），Tauri updater 插件初始化 panic，应用无法启动
- **截图**: FAIL — `bu.screenshot()` 3 次均 COMMAND_TIMEOUT（浏览器自动化截图在当前环境受限），文本证据充分替代

**详细证据**: `.scratch/desktop-rewrite/golden-path/GOLDEN-PATH-EVIDENCE.md`

**浏览器验证摘要**:
- 初始页面: title=InsightForge，侧栏 8 按钮，工作台视图正常
- 草稿视图: 标题/新建按钮/草稿列表/编辑器占位符正常渲染，错误提示正常工作
- 检索视图: 无来源时正确显示 emptySource 提示
- 引用视图: 无 activeDraft 时正确显示 emptyDraft 提示（验证共享状态）
- 控制台: 0 errors, 0 warnings

### 6. 补测试：前端共享状态 provider/store 测试 + references Rust 持久化测试

**结果: PASS**

**前端测试** (`src/AppStateContext.test.tsx`, 8 tests):
- DraftsModelContext provider/consumer round-trip
- useDraftsModel 在 provider 外抛明确错误
- AppStateContext 默认值（空 sources + null activeDraft）
- AppActionsContext 默认 no-op actions 可调用
- useDrafts 初始状态
- useDrafts actions.refresh 调用 drafts_list 并更新列表
- useDrafts actions.open 设置 activeDraft
- useDrafts actions.delete 在 activeDraft null 时 no-op

**Rust 测试** (`crates/core/src/references.rs` `#[cfg(test)] mod tests`, 8 tests):
- create_reference_snapshots_fragment_and_persists
- create_reference_with_nonexistent_fragment_returns_error
- list_references_returns_created_snapshots_ordered_by_created_at_desc
- list_references_for_unknown_draft_returns_empty
- adopt_marks_reference_as_adopted
- mark_source_deleted_preserves_snapshot_and_flags_source_unavailable
- mark_source_deleted_only_affects_matching_source
- long_text_snippet_is_truncated_with_ellipsis

**可复跑命令**: `bun run test` / `cargo test --workspace`

## P0/P1 缺陷修复清单

| ID | 缺陷 | 修复位置 | 状态 |
|----|------|---------|------|
| P0-A | App.tsx 未 import useState | `src/App.tsx:1` | FIXED |
| P0-B | App.tsx 用未定义的 activeDraft | `src/App.tsx` references pane → `draftsModel.activeDraft` | FIXED |
| P1-C | 共享状态未闭环（双 hook + no-op actions） | `AppStateContext.tsx` + `App.tsx` + `DraftsView.tsx` | FIXED |
| P1-D | DraftsView localDrafts 未用 + drafts 未定义 + d 隐式 any | `src/drafts/DraftsView.tsx` | FIXED |
| P1-E | ReferencesView/SearchView appState 解构未用 | `src/references/ReferencesView.tsx`（移除）; `src/search/SearchView.tsx`（改为使用 activeDraft） | FIXED |
| P1-F | SearchView 无固定入口 + ReferencesView 不调 references.create | `src/search/SearchView.tsx` + 后端 type bug fix | FIXED |

## 额外修复（任务范围内必要的附带修复）

1. **后端 type bug**: `FragmentSnapshot.source_file_id` String→i64（references_create 死代码根因）
2. **useDrafts actions.refresh**: 暴露内部 refresh 回调（AppActionsContext.refreshDrafts 需要）
3. **useSources actions.refresh**: 暴露内部 refresh 回调（AppActionsContext.refreshSources 需要）
4. **e2e/golden-path.spec.ts**: 移除未用的 `getInstallerVersion` 函数和最后一个测试中未用的 `page` 参数（预存 lint 错误）
5. **playwright.config.ts**: 移除未用的 `INSTALLER_PATH` 常量（预存 lint 错误）

## i18n 最小 patch 建议（locale JSON 为锁定文件，由主控统一接线）

SearchView 新增了 3 个 i18n key，当前运行时会 fallback 到 key 字符串。建议主控在合并时添加到 `src/i18n/locales/{zh-CN,en-US}.json` 的 `search` 节：

```json
"search": {
  "pinToDraft": "固定到草稿",
  "pinSuccess": "已固定",
  "pinNoDraft": "请先打开草稿"
}
```

```json
"search": {
  "pinToDraft": "Pin to draft",
  "pinSuccess": "Pinned",
  "pinNoDraft": "Open a draft first"
}
```

## 遗留缺口 / BLOCKED 项

1. **原生 Tauri GUI Golden Path**: BLOCKED — `tauri.conf.json` 的 `plugins.updater` 配置类型错误（锁定文件）。建议主控修复为 sequence 类型后重新执行 `bun run tauri dev` 端到端验证。
2. **截图证据**: FAIL — 浏览器自动化 `bu.screenshot()` 在当前环境超时。文本证据（页面标题、DOM 文本、控制台消息）已充分验证 UI 渲染。
3. **i18n key**: 3 个新 key 暂未加入 locale JSON（锁定文件），运行时 fallback 到 key 字符串，不影响功能。
4. **SearchView 固定按钮的端到端验证**: 因无 Tauri 运行时（浏览器环境 invoke 未定义），无法在浏览器中真实调用 `references_create`。后端已通过 8 个 Rust 测试验证 create/list/adopt/mark_source_deleted 全链路。

## 锁定文件确认（未修改）

- `src-tauri/src/lib.rs` — 未修改
- `src-tauri/tauri.conf.json` — 未修改（其 updater 配置错误为预存问题，记为 BLOCKED）
- `src/i18n/locales/zh-CN.json` — 未修改
- `src/i18n/locales/en-US.json` — 未修改
- 06 原子保存逻辑、04 watcher、09 AI 功能 — 未修改

## 禁止项确认

- 未 push 到远端
- 未建 PR
- 未发布 / 未创建 GitHub Release
- 未 merge 进 integration
- 未删/改其他 worktree 或用户文件
- 仅 1 次 Finalizer 会话
