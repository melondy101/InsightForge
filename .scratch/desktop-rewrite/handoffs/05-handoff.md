# Handoff · Ticket 05 — 三栏工作台与 Markdown 编辑器
- Ticket: .scratch/desktop-rewrite/issues/05-workspace-editor.md
- Baseline SHA / 分支 / worktree 路径: 10ad44b / ticket/05-workspace-editor / D:\Develop\wt\InsightForge\05-workspace-editor
- Attempt: Builder 1（此前失败会话：none）

## 改动摘要
- **提取纯函数**（`src/editor/context.ts`，18 测）：`extractEditorContext(doc, {anchor, head})` 返回光标/选区所在段落、前后相邻段落、标题大纲（含层级路径，栅栏内 `#` 不算标题）；另有 `paragraphIndexAt`（所在段全文序号）与 `headingPathAt`（光标处标题链）供状态条与 09 出站包共用。段落 = 连续非空行块（软换行同段）；空行光标 → current=null；跨段选区 → 扩展为触碰块整段 span；文档边界 → null。
- **文件文本归一化**（`src/editor/file-text.ts`，15 测）：`analyzeFileText`/`restoreFileText`——BOM（U+FEFF）剥除还原、CRLF/LF/CR 记录还原（取首个行分隔符为文件风格）、混合行尾有编辑后统一为检测风格（VS Code 语义）。
- **EditorAdapter 适配层**（`src/editor/adapter.ts`，13 测）：窄接口 `openDocument/getValue/setValue/getSelection/setSelection/extractContext/onChange/onSelectionChange/mount/unmount/focus/destroy`，CodeMirror 6 藏在其后，不暴露任何 CM 类型。round-trip 关键设计：`getValue()` 在「当前文档与打开基线逐字符一致」时直接返回原始文本（对混合行尾/BOM 一切形态逐字节无损），有改动才走 `restoreFileText`（按文件特征还原行尾与 BOM）。适配器 headless 可用（仅依赖 `@codemirror/state` 文档模型），`mount` 后由 `EditorView` 接管——语料与契约测试因此无需 DOM、不耦合视图。
- **round-trip 语料**（`src/editor/roundtrip.test.ts`，20 测）：19 组语料（见下）打开→不修改→序列化，Buffer 逐字节相等全绿；外加「写入真实临时文件→打开→写回」两文件 `Buffer.compare=0`。
- **三栏工作台**（`src/workspace/layout.ts` 11 测 + `WorkspaceView.tsx` + `App.tsx` + `styles.css`）：`applyDividerDrag`（拖拽语义：中栏触线时分隔条恰好停在边界，侧栏下限 48px）与 `clampPaneWidths`（resize 语义：中栏抬回 35% 时从较大侧栏让位，变宽时侧栏保像素、中栏吸收余量）两个纯函数约束；UI 为左栏（证据架/检索占位）+ 可拖拽分隔条×2 + 中栏编辑器 + 右栏占位，02 的来源管理入口原样保留。编辑器状态条实时显示「第 N 段 + 标题路径」（提取纯函数的 UI 可见冒烟）。
- **i18n**：新增 `workspace.*` 命名空间（zh-CN/en-US 对称：paneLeft/paneRight/editorPlaceholder/status.paragraph|blankLine|emptyDoc|noHeading）；移除被真实 UI 取代的 `shell.editor`/`shell.evidenceShelf` 占位键，`shell.sidebar` 保留并用作导航 aria-label；i18n 键一致性测试同步更新。交互元素带 `data-el`（workspace-pane-left/center/right、divider-left/right、editor-host、editor-status 等）。
- **新增 npm 依赖及理由**（bun.lock 相应更新）：
  - `@codemirror/state 6.7.1`——文档模型与事务，适配器 headless 测试与 round-trip 的基础；
  - `@codemirror/view 6.43.9`——EditorView 挂载、真实输入/IME 组合处理、lineWrapping、placeholder、updateListener；
  - `@codemirror/language 6.12.4`——`syntaxHighlighting(defaultHighlightStyle)` 语法高亮运行时；
  - `@codemirror/lang-markdown 6.5.2`——Markdown 语法 + `markdownKeymap`（Enter 列表续行等）；
  - `@codemirror/commands 6.11.0`——`history`（撤销）与 defaultKeymap/historyKeymap；
  - devDep `@types/node`（仅 round-trip 测试用 `node:fs/promises` 与 `Buffer` 做 Buffer 级断言）。

## 验收标准 → 证据映射
| 票据验收标准 | 证据（命令/文件/截图路径 + 关键原始输出节选） |
| --- | --- |
| 1. 三栏布局可调，正文宽度下限 35%；编辑器藏在 `EditorAdapter` 后 | 纯函数：`src/workspace/layout.test.ts` 11 测（过拖在中栏触线时恰好停住 `expect(next.center).toBe(total * CENTER_MIN_RATIO)`、总和恒等、侧栏 ≥48px、resize 再约束）。UI 实测（GP step-05/12/13）：初始占比 `0.6` → 向右过拖左分隔条 700px 后 `0.35`（被约束停住，截图 `evidence/05-gp-03-drag-left-clamped.png`）。适配层：`src/editor/adapter.ts` 是唯一含 CM 导入的文件，接口与实现文件均无 CM 类型泄漏（`grep -l "@codemirror" src --include=*.ts*` 仅 adapter.ts 与 WorkspaceView.tsx 的扩展装配处） |
| 2. Markdown round-trip 语料测试全绿：打开→不修改→保存逐字节一致 | `bun run test`：`Tests 84 passed (84)`，其中 round-trip 语料 20 测。语料 19 组：纯 ASCII、中文长文、CRLF、LF、CR（经典 Mac）、BOM 开头、无尾换行、尾随空格（硬换行）、Tab 缩进、嵌套列表（含任务列表）、代码栅栏（```/~~~）、表格、引用块（嵌套）、emoji 与代理对（🎉👨‍👩‍👧‍👦☝🏻）、行内代码与强调、混合行尾、空文件、只有换行的文件。断言 `Buffer.from(value,'utf8').equals(Buffer.from(raw,'utf8'))`；另有一测写真实临时文件再 `Buffer.compare(readFile(original), readFile(saved)) === 0`。守卫测试锁语料覆盖三种行尾（防删样例静默缩水） |
| 3. Windows 中文输入法长文连续输入无丢字/错位（M0 第 1 项，手动验证折叠于此） | 方法与逐条「预期 vs 实际」见下节与 `evidence/05-gp-steps.log`。核心结果（GP step-19~27）：IME 组合窗模拟（`Input.imeSetComposition` 拼音 preEdit → 候选确认 → `Input.insertText` 提交）2 次 + CDP `Input.insertText` 直插 4 段，共 570 字符（含标点/书名号/全角破折号/数字 2026、0517/英文 CodeMirror、WebView2/emoji 🎉🚀）；step-26 `PASS: doc 570 chars vs inserted 570 chars`（`getValue()` 与累计输入逐字符相等——无丢字/无错位/无重复）；step-27 `{len:570, cursorAtEnd:true}`。截图 `evidence/05-gp-04-long-chinese-typed.png`、`05-gp-05-extract-heading.png` |
| 4. 能提取光标/选区前后段落与标题大纲（供 09 出站包） | 纯函数层 18 测（含光标段首/段中/段尾、跨段选区、反向选区、空行光标、文档开头/结尾边界、空文档、多级标题路径、栅栏内 `#` 排除）；adapter 接线测试 2 测（含 CRLF 文件打开后偏移坐标系一致）。GP step-30 实测真实编辑器：`{"current":"正文在第一章。","outline":[{"level":1,"text":"第一章 · 验证结论","path":[]}]}`；状态条同屏显示「第 3 段 · 第一章 · 验证结论」（截图 `05-gp-05-extract-heading.png`） |

## 运行过的命令与原始结果
> 工具链：cargo/rustc/clippy 1.94.0（rustup）。**每条 cargo/tauri 命令前均 `export PATH=/c/Users/20145/scoop/persist/rustup/.cargo/bin:$PATH`**（规避 scoop shim 1.90 与 Tauri 混编崩溃，见 01 handoff）。

1. `bun install`（新增 @codemirror/* 与 @types/node）— 退出码 0，`installed @codemirror/state@6.7.1 … 21 packages installed`
2. `cargo fetch` — **退出码 0**
3. TDD 红灯记录（每行为先红后绿，`bunx vitest run <file>`）：`context.test.ts` 首测红（模块不存在）→ 绿；中途两次红灯均为**测试自身错误**（软换行段落语义写错、混合行尾恒等期望写错），按定义修正测试后绿；`adapter.test.ts` 红灯抓到**真实设计缺陷**：setValue 曾重设基线导致「编辑路径」与「打开新文件」不可区分、打开文件的行尾特征丢失——修正语义为「setValue=编辑（保留文件特征与基线）；openDocument=打开（重设基线与特征）」后绿；`roundtrip.test.ts` 曾因 `EditorState.updateListener`（实为 `EditorView.updateListener`）与 `state.update()` 返回 `Transaction`（取 `.state`/`.docChanged`/`.selection`）两次红灯，修正后 20/20 绿。
4. **`bun run gate` 一键全链 — 退出码 0**，完整原始输出 `evidence/gate-full-05.log`（8 段门禁逐段通过）：
   - `bun run lint` 0 error / 0 warning；`bunx tsc --noEmit` 无输出
   - `bun run test`：`Test Files 7 passed (7); Tests 84 passed (84)`（i18n 4 + progress 3 + context 18 + file-text 15 + roundtrip 20 + adapter 13 + layout 11）
   - `bun run build`：`✓ built in 333ms`（`dist/assets/index-*.js 757.90 kB │ gzip: 256.21 kB`——CodeMirror 全量入库所致，vite 给出 chunk>500kB 提示，退出码 0，非告警阻塞）
   - `cargo fmt --check` 无输出；`cargo clippy -- -D warnings` 通过；`cargo test`：`18 passed（core 单元）+ 9 passed（repository）+ 3 passed（schema）+ 18 passed（sources）`，0 failed——**Rust 侧零改动，测试与 02 交付时完全一致**；`cargo build`：`Finished dev profile in 8.14s`
   - 日志中仅有的 "warning" 字样：命令回显 `-D warnings` 旗标与 vite chunk 体积提示，均非编译告警
5. **`bun run tauri dev`（后台，`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`）**，日志 `evidence/tauri-dev-05.log`：
   ```
   Running `D:\...\target\debug\insightforge-desktop.exe`
   [insightforge] database ready: path=C:\Users\20145\AppData\Roaming\app.insightforge.desktop\insightforge.sqlite sqlite=3.53.2 journal_mode=wal applied_migrations=3
   ```
6. **Golden Path（实跑两轮，最终轮 34 步全符合预期、无 ERROR）**：日志 `evidence/05-gp-steps.log`（首轮 `05-gp-steps-attempt1.log`：拖拽与组合提交两动作驱动方式有缺陷，见「风险」；探针定位后修复驱动重跑）。关键节选：
   ```
   step-05:eval -> 0.6（初始中栏占比）
   step-07:waitFor -> true（来源管理视图渲染，02 回归）
   step-13:eval -> 0.35（过拖 700px 后中栏恰停在 35% 下限）
   step-19:composition -> doc tail="中文输入测试"（IME 组合提交）
   step-26:expectValueEqualsInserted -> PASS: doc 570 chars vs inserted 570 chars
   step-27:eval -> {"len":570,"cursorAtEnd":true}
   step-30:eval -> {"current":"正文在第一章。","outline":[{"level":1,"text":"第一章 · 验证结论","path":[]}]}
   ```
   截图：`05-gp-01-three-pane.png`（三栏+状态条「空文档」）、`05-gp-02-sources-view.png`（02 来源管理视图完好）、`05-gp-03-drag-left-clamped.png`（35% 触线）、`05-gp-04-long-chinese-typed.png`、`05-gp-05-extract-heading.png`（状态条「第 3 段 · 第一章 · 验证结论」）。
7. **干净退出验证**：带 CDP 调试口时窗口关闭后宿主进程因调试服务器存活（见风险）；**去掉该 env 复测：`CloseMainWindow()` 后 1 秒内进程退出（`exited-after-1s`），tauri dev 自然退出码 0**——与应用自身关闭行为一致（01/02 同款验证）。
8. 探针脚本（留档 `handoffs/scripts/`）：`probe-drag-05.ts`（证明 CDP 鼠标序列可驱动分隔条且约束生效 `center ratio now: 0.35`）、`probe-ime-05.ts`/`probe-ime-commit-05.ts`（确定 WebView2 CDP 的组合提交序列）。

## 风险 / 限制 / 已知问题
- **IME 验证方法与残余风险（必读）**：本票无法驱动真实 OS 级微软拼音（需要真实键盘输入与 IME 会话，CDP 无此通道）。采用的等价手段是 Chromium 输入管线的组合模拟：`Input.imeSetComposition`（浏览器原生 IME 组合 API，产生真实 trusted compositionstart/update 事件与 DOM 组合区）+ `Input.insertText`（CDP 文档明示「模拟非按键文本插入，如 IME 上屏」）。这覆盖了 CM6 处理组合文本建立/替换/提交与批量 IME 上屏的代码路径，且 570 字混排长文逐字符无损。**残余风险**：真实 IME 的键盘预处理（key events→组字串生成）、候选窗 UI、组音过程中的多次部分提交等 OS 侧行为未被覆盖；建议 15 号票（Windows E2E）在真实 IME 下复核一次。
- **WebView2 CDP 组合语义坑（探针实证）**：`Input.insertText('')` = **取消**当前组合（组合文本被清除），不是提交；正确提交序列为 `imeSetComposition(preEdit) → imeSetComposition(commit) → insertText(commit)`（恰好上屏一份）。首轮 GP 驱动即踩此坑（`05-gp-steps-attempt1.log` step-19/20），已修复并在最终轮验证。另：合成 CompositionEvent + 手改 DOM 的模拟路径会让 CM6 视图内部状态不一致（后续 dispatch 抛 TilePointer 错误）——探针越界行为，CM 明确禁止组合期间外部直接改 DOM；该路径未用于任何验收证据。
- **带 CDP 调试口时应用不随窗口关闭退出**：`--remote-debugging-port=9222` 下窗口关闭后宿主进程仍存活（调试服务器保持），需结束 dev 会话；**无该 env 时 1 秒内干净退出（已实测）**——仅调试会话伪影，不影响用户场景（用户运行不带调试口）。
- **GP 收尾说明**：最终轮驱动结束后，应用以 `taskkill`（WM_CLOSE）+ 结束 dev 会话方式关闭；进程存活为上述调试口伪影，非崩溃。全部 GP 断言在关闭前完成并留档。
- 视图切换（工作台↔来源管理）会重建编辑器，未保存的临时内容随视图卸载丢失——草稿的打开/保存/持久化是 06 号票领域（`drafts.open/save` + 基线），本票接口（`openDocument/getValue`）已为其备好。
- 右栏宽度 GP 实测 47px 而纯函数下限为 48px：`applyDividerDrag` 按 `clientWidth`（整数）约束，浏览器按小数像素布局，±1px 亚像素渲染差；35% 硬约束实测恰为 0.35 无偏差。
- 编辑器 placeholder 在运行时切换语言后不会更新（随首次挂载语言固定）；重启生效。运行时语言切换本就是后续票据范围。
- `bun run build` 产物 757.9 kB（gzip 256 kB）：CodeMirror 5 包全量同步打包；未做代码分割（spec 无此要求，桌面本地加载无网络成本），后续票据如在意可 `manualChunks` 拆分。
- 适配器契约测试（round-trip/提取/通知/选区）全部 headless 运行、不挂 DOM——符合「换编辑器实现测试仍绿」的要求（它们只测 `EditorAdapter` 接口契约与纯函数）；EditorView 的真实挂载/输入行为由 GP 在真实 WebView2 窗口验证。
- 沿袭 01/02 已知项：CSP 仍为 null（建议 03 号票收紧）；本票未改 Rust，迁移仍为 0001–0003。
- 新增依赖仅 @codemirror/* 5 包 + devDep @types/node；无 `as any`、无类型豁免、无删测试、无弱化断言（i18n 测试 2 例因占位键被真实 UI 取代而同步改写，断言强度不变）。

## 推荐下一步
主控可放行 06（草稿打开/原子保存/冲突）——`EditorAdapter.openDocument/getValue/setValue` 即为其 seam，文件文本归一化已保证保存侧字节语义；Reviewer 建议重点复核 `adapter.ts` 的基线快路径（`getValue` 的 `doc === baseline → originalRaw`）与 `file-text.ts` 的混合行尾语义是否符合预期，并在合并前跑一次 `bun run gate` 复核。
