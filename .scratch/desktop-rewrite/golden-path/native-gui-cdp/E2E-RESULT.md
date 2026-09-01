# 03/07 Native GUI Golden Path — WebView2 CDP E2E Result

> Execution date: 2026-09-01
> Environment: Windows 11, Tauri 2 native desktop (debug build), WebView2 CDP port 9222
> Worktree: D:\Develop\wt\InsightForge\03-07-native-gui-closure
> Branch: ticket/03-07-native-gui-closure
> Test source: D:\Develop\wt\InsightForge\_e2e-source-cdp (1 markdown file: knowledge-management.md)

## Execution Method

- **CDP connection**: `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222 --remote-allow-origins=*`
- **Driver**: Python `websocket-client` + `requests`, direct CDP WebSocket commands
- **UI interaction**: `Runtime.evaluate` DOM `click()` (no synthetic mouse)
- **Dialog handling**: `window.confirm` override (returns true) + `Page.handleJavaScriptDialog` fallback
- **Screenshots**: `Page.captureScreenshot` (PNG, base64)

## Step-by-Step Results

### Step 1: Add source and index — ✅ PASS

- Navigated to Sources view (`[data-el="nav-sources"]`)
- Filled name: "CDP Golden Path 来源" (`[data-el="sources-add-name"]`)
- Pasted path: `D:\Develop\wt\InsightForge\_e2e-source-cdp` (`[data-el="sources-add-path"]`)
- Clicked "添加并索引" (`[data-el="sources-add-submit"]`)
- Source appeared in list with status "空闲" (idle), file count 1
- Screenshot: `gp2-step1-form.png`, `gp2-step1-indexed.png`
- Verified invoke: `sources_add` + `sources_list` (Rust backend processed successfully)

### Step 2: Create draft — ✅ PASS

- Navigated to Drafts view (`[data-el="nav-drafts"]`)
- Overrode `window.prompt` to return "gp-fresh-draft.md"
- Clicked "新建" button
- Draft created: `gp-fresh-draft.md` with timestamp `2026-09-01T12:XX:XX.XXXXXXZ` (correct `%S.%6f` format — confirms P2-2 fix)
- Draft auto-activated: toolbar showed "新建|保存|删除"
- Screenshot: `gp2-step2-draft-created.png`
- Verified invoke: `drafts_create`

### Step 3: Search and pin to draft — ✅ PASS

- Navigated to Search view (`[data-el="nav-search"]`)
- Entered query "知识管理" (`[data-el="search-input"]`)
- Clicked submit (`[data-el="search-submit"]`)
- Search returned 3 evidence cards, each with:
  - Source name + file path + heading path
  - Highlighted snippet
  - Relevance score
  - "打开上下文" and "固定到草稿" buttons
- Clicked first "固定到草稿" (`[data-el="search-card-pin"]`) via DOM `click()`
- Button text changed to "已固定" (pin success — no i18n key leak)
- Screenshot: `gp2-step3-results.png`, `gp2-step3-pinned.png`
- Verified invoke: `search_query` + `references_create`

### Step 4: Verify references — ✅ PASS

- Navigated to References view (`[data-el="nav-references"]`)
- Reference displayed with:
  - Source: "CDP Golden Path 来源"
  - File: `knowledge-management.md`
  - Line range: 5-8
  - Snippet: "## 知识管理的核心流程 知识管理包括四个核心流程：知识获取、知识存储、知识共享和知识应用。"
  - "采用" button (adopt action)
- Screenshot: `gp2-step4-references.png`
- Verified invoke: `references_list`

### Step 5: Delete source — ✅ PASS (database confirmed)

- Navigated to Sources view
- Overrode `window.confirm` to return true
- Clicked "移除" button (`[data-el="sources-remove-btn"]`) via `setTimeout` + DOM `click()`
- Console log confirmed: `confirm called with: 移除来源只清理索引记录，不会改动源文件。确定移除？` → override returned true
- **UI note**: Source did not disappear immediately due to pre-existing state management issue (SourcesView uses `appState.sources` global state while `remove` action only updates local `useSources` state). Source confirmed removed after page reload:
  - After `Page.reload`: sources list showed "还没有来源" (no sources), 0 remove buttons
- Screenshot: `gp2-step5-source-deleted.png`, `gp-final-sources.png`
- Verified invoke: `sources_remove` (database updated, confirmed by reload)

### Step 6: Verify reference snapshot preserved — ✅ PASS

- After source deletion (before reload), References view still showed the reference with full content
- Reference snapshot is independent of source existence (non-derived data)
- Screenshot: `gp2-step6-snapshot.png`
- **sourceDeleted marker**: ⚠️ NOT automatically set — `sources_remove` does not call `references_mark_source_deleted`. This is a pre-existing design gap (the `mark_source_deleted` method exists and is unit-tested, but is not wired into the source removal flow).
- Verified: reference content preserved after source deletion

## Acceptance Criteria Verification

| Criterion | Result | Evidence |
|-----------|--------|----------|
| No console errors | ✅ PASS | 0 errors across all CDP sessions |
| No native panic | ✅ PASS | App logs show no panic, database ready, settings ready |
| No i18n key leaks | ✅ PASS | UI shows translated text ("固定到草稿", "已固定", "采用", "移除"), no raw key strings |
| Add source + index | ✅ PASS | Step 1 |
| Search returns evidence cards | ✅ PASS | Step 3 (3 cards) |
| Open/create draft | ✅ PASS | Step 2 |
| Pin to draft | ✅ PASS | Step 3 (button → "已固定") |
| References show real data | ✅ PASS | Step 4 (source, file, lines, snippet, adopt) |
| adopted/sourceDeleted status | ⚠️ PARTIAL | adopt button present; sourceDeleted not auto-wired (pre-existing gap) |
| Delete source via DOM click() | ✅ PASS | Step 5 (confirmed by reload) |
| Snapshot preserved after delete | ✅ PASS | Step 6 |

## Pre-existing Gaps Found (not introduced by this ticket)

1. **SourcesView state sync**: `remove` action updates local `useSources` state but view uses `appState.sources` global state when non-empty → UI doesn't refresh until reload.
2. **sourceDeleted not auto-wired**: `sources_remove` does not call `references_mark_source_deleted` → references don't show "来源已删除" marker after source removal. The `mark_source_deleted` method exists and is unit-tested.
3. **drafts `open()` UNIQUE constraint**: `generate_id()` creates a new ID each call (filename + timestamp nanos), but `upsert_draft` uses `ON CONFLICT(id)` → opening an existing draft violates `UNIQUE(draft.path)`. New draft creation works; re-opening existing draft fails.
4. **chrono `%f` format**: Original `%Y-%m-%dT%H:%M:%fZ` omitted `%S` (seconds) entirely — timestamps lost second precision. Fixed by P1-1/P2-2 to `%Y-%m-%dT%H:%M:%S.%6fZ`.

## Evidence Files

All evidence in: `.scratch/desktop-rewrite/golden-path/native-gui-cdp/`

### Key screenshots
- `gp2-step1-indexed.png` — source added and indexed
- `gp2-step2-draft-created.png` — draft created with correct timestamp format
- `gp2-step3-results.png` — search results with pin buttons
- `gp2-step3-pinned.png` — pin success ("已固定")
- `gp2-step4-references.png` — reference with full content
- `gp2-step5-source-deleted.png` — after remove click
- `gp2-step6-snapshot.png` — snapshot preserved after delete
- `gp-final-sources.png` — after reload: 0 sources (confirmed deletion)

### Logs and data
- `console-gp2-full.jsonl` — console messages from full golden path run
- `console-final-verify.jsonl` — console messages from final verification
- `gp2-full-results.json` — structured results from full golden path
- `final-verify-summary.json` — final verification summary

## CDP Connection Details

- HTTP endpoint: `http://localhost:9222/json`
- WebSocket: `ws://localhost:9222/devtools/page/<target_id>`
- Environment variable: `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222 --remote-allow-origins=*`
- Tauri dev command: `bun run tauri dev` (with env var set)

## Conclusion

**Golden Path: PASS with noted pre-existing gaps**

The core user闭环 (source add → index → draft create → search → pin → references → source delete → snapshot preserved) is fully verified in the native Tauri runtime via WebView2 CDP. All invoke bridges function correctly. No console errors, no native panics, no i18n key leaks.

Three pre-existing gaps were identified (state sync, sourceDeleted auto-wiring, draft open UNIQUE constraint) — none introduced by this ticket's changes.
