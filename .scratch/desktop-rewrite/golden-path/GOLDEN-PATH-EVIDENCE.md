# Golden Path 验证证据 — 03/07 Finalizer

## 环境信息
- OS: Windows (本机 PowerShell)
- bun: 1.3.11
- cargo/rustc: 1.90.0
- node: v22.23.2
- 工作树: D:\Develop\wt\InsightForge\03-07-finalizer
- 分支: ticket/03-07-finalizer
- 基线: integration/desktop-rewrite @ 1c764d3

## 尝试 1: bun run tauri dev（原生 Tauri GUI）

**结果: BLOCKED — 预存配置错误（锁定文件，不可改）**

执行命令: `bun run tauri dev`

输出摘要:
```
Running BeforeDevCommand (`bun run dev`)
VITE v8.2.2  ready in 605 ms
➜  Local:   http://localhost:1420/
Running DevCommand (`cargo run --no-default-features --color always --`)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.00s
Running `D:\Develop\wt\InsightForge\03-07-finalizer\target\debug\insightforge-desktop.exe`

thread 'main' panicked at src-tauri\src\lib.rs:592:10:
error while running tauri application: PluginInitialization("updater",
"Error deserializing 'plugins.updater' within your Tauri configuration:
invalid type: map, expected a sequence")
```

**根因**: `src-tauri/tauri.conf.json` 中 `plugins.updater` 字段类型错误（map 而非 sequence），导致 Tauri updater 插件初始化失败，应用无法启动。
**为何不修**: `tauri.conf.json` 是主控锁定文件，03/07 Finalizer 任务卡明确禁止修改。
**影响**: 原生 Tauri 桌面 GUI 无法启动，无法进行端到端的原生 GUI 驱动验证。

## 尝试 2: bun run dev（Vite dev server + 浏览器驱动）

**结果: PARTIAL PASS — UI 渲染与交互验证通过，Tauri 命令因无原生运行时而失败（预期）**

执行命令: `bun run dev`
- Vite dev server 成功启动: `http://localhost:1420/`
- 浏览器成功打开并导航到应用

### 初始页面验证
- page_info: url=http://localhost:1420/, title=InsightForge, 1050x955
- 页面文本:
  ```
  织知 InsightForge
  工作台 / 来源管理 / 草稿 / 引用 / 思考卡 / 检索 / 设置
  证据架 / 检索
  开始写作…
  空文档
  无标题
  预留
  ```
- 侧栏导航 8 个按钮全部渲染
- 默认工作台视图正常渲染

### 交互验证：草稿视图
点击 `[data-el="nav-drafts"]` 后页面文本:
```
草稿
新建
Cannot read properties of undefined (reading 'invoke') 关闭
草稿列表
还没有草稿。
打开或新建草稿以开始写作。
```
- DraftsView 正常渲染（标题、新建按钮、草稿列表、编辑器占位符）
- 错误提示正常工作（浏览器无 Tauri 运行时，invoke 未定义 — 预期行为，非 bug）
- 共享状态模型正常注入（DraftsView 从 DraftsModelContext 消费状态）

### 交互验证：检索视图
点击 `[data-el="nav-search"]` 后页面文本:
```
检索
请先在来源管理中添加并索引一个目录，再使用检索。
```
- SearchView 正常渲染
- 无来源时正确显示 emptySource 提示（useSources 在浏览器中 invoke 失败 → sources 为空 → defaultSource 为 null）
- 固定到草稿按钮在有来源+有草稿时才会显示（当前条件不满足，符合预期）

### 交互验证：引用视图
点击 `[data-el="nav-references"]` 后页面文本:
```
引用
请先在草稿中打开或创建一个文件，再查看引用。
```
- ReferencesView 正常渲染
- 无 activeDraft 时正确显示 emptyDraft 提示（验证共享状态：App 从 draftsModel.activeDraft 读取，DraftsView 的 useDrafts 与 App 共享同一份状态）

### 控制台检查
- console_messages: 7 条
- Errors: 0
- Warnings: 0
- 无 React hook 错误、无 context 错误、无未处理异常

### 截图
- 尝试 `bu.screenshot()` 3 次，均 COMMAND_TIMEOUT（浏览器自动化截图在当前环境受限）
- 文本证据充分替代截图验证

## 结论

| 验证项 | 结果 | 说明 |
|--------|------|------|
| Vite dev server 启动 | PASS | localhost:1420 正常 |
| 应用渲染（React mount） | PASS | title=InsightForge，侧栏+工作台正常 |
| 侧栏导航交互 | PASS | 4 个视图切换均正常 |
| DraftsView 渲染 | PASS | 共享状态注入正常 |
| SearchView 渲染 | PASS | 无来源提示正常 |
| ReferencesView 渲染 | PASS | 无草稿提示正常（验证共享状态） |
| 控制台 0 error/warning | PASS | React 代码干净 |
| 原生 Tauri GUI 启动 | BLOCKED | tauri.conf.json updater 配置类型错误（锁定文件） |
| 端到端 Tauri 命令调用 | N/A | 需原生 Tauri 运行时，浏览器环境不支持 |
| 截图证据 | FAIL | bu.screenshot() 超时，文本证据替代 |

**Golden Path 总体判定: PARTIAL PASS**
- 前端 UI 渲染、共享状态、交互导航均通过浏览器真实驱动验证
- 原生 Tauri GUI 因锁定文件的预存配置错误无法启动，记为 BLOCKED
- 建议主控在合并前修复 tauri.conf.json 的 plugins.updater 配置，然后重新执行原生 GUI Golden Path
