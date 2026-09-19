# Changelog

本文件记录当前桌面版（0.1.0）的用户可见变更。旧 Web 版历史不再作为当前实现的变更记录。

## [Unreleased]

### Added

- Tauri 2 + Rust 本地核心、React/Vite 前端与 SQLite/FTS5 索引。
- 来源管理、关键词检索、真实 Markdown 草稿、引用快照、陪练、思考卡及 HTML/微信公众号剪贴板导出。
- `sources_await_index_committed`：等待 backfill 与 FTS 提交完成后才开放搜索，避免事件投递窗口造成的空结果。
- 草稿冲突三选一（保留本地 / 采用外部 / 另存为），外部修改不再静默覆盖。
- 孤儿草稿自动登记：`list()` 发现磁盘 `.md` 即登记，不再静默丢弃。
- 原子保存临时文件唯一命名 + 启动/列表时清理残留，降低崩溃或杀软干扰下的残留风险。
- 数据库打开完整性校验：损坏库备份为 `.corrupt.bak` 后重建，应用可安全降级而非崩溃。
- 诊断视图：脱敏诊断快照（版本/平台/SQLite 版本/迁移/完整性/表计数，不含密钥与正文）与结构化事件日志，支持复制/导出，便于求助。
- E2E 黄金路径规格（7 条主路径，受 WebView2 探针门控为 BLOCKED，不伪 PASSED）与 Windows 手动验证脚本。
- 发布 Runbook 与 fail-closed 构建（缺失签名/端点/公钥时 release 构建明确失败；dev 不受影响）。
- **陪练视图**：卡住时发起、按三轮推进、可接续该草稿最近一次会话；只出提示与问题，界面上没有「插入正文」的补全按钮。
- **来源同步间隔真正生效**：5/15/30 分钟档位（默认 15）到点自动补扫，不再只是写进设置。
- **错误文案本地化**：每个命令错误码在 zh-CN / en-US 都有一句话说明，用户不再看到 `io_error` 这类裸码。
- **一份核心、三类 runtime 的客户端侧**：命令统一走 `CommandClient` seam（桌面 Tauri / 网页 HTTP / 测试 fake）；
  同一份 React 代码可构建网页静态站，配套 `deploy-web.yml`「部署即更新」（未配服务端地址时主动失败，不发壳）。
  注意：`insightforge-server` 本体还没写，网页没有后端可连。
- **跨设备同步的地基**：设备登记与撤销（撤销不可逆）、`rev` + `origin_device` 定序、
  同步载荷改为 `SyncBundle` 类型——本机路径与保存基线在类型上就出不了境。
- 界面样式补齐：视图里 68 个「用了但样式表没定义」的类全部落实（此前这些元素实际未样式化），
  并清掉 35 处无人引用的死规则；两个方向由 `src/styles.test.ts` 双向守着。
- 开发体验：Vite 装上 `@vitejs/plugin-react`，dev 模式恢复 Fast Refresh（改一个组件不再整页重载）。

### Fixed

- 草稿重开时的唯一约束冲突。
- 删除来源后引用快照的来源不可用状态和来源列表同步。
- **检索证据卡的片段曾按 HTML 注入渲染**（`dangerouslySetInnerHTML`）：知识库文件里出现的标签与
  `onerror` 会在界面里执行。现在片段一律按文本渲染，只有 FTS5 的 `<mark>` 变成高亮节点，
  并由一条全仓扫描测试禁止再出现 `dangerouslySetInnerHTML`。
- 诊断视图的版本号不再绕过命令通道直连 Tauri 专有 API（浏览器 runtime 下必挂）；
  版本取自 workspace `Cargo.toml` 单一真源，CI 拦截 tag / Cargo / package.json 漂移。

### Release status

- 桌面重写已合入 `main`；上述 2026-09-19 一轮的提交在分支 `feat/ui-v1-implement`，**尚未推送、尚未合回**。
- 公开发布尚未验收：没有签名安装包、公开 HTTPS updater 或安装→升级 E2E 证据。
