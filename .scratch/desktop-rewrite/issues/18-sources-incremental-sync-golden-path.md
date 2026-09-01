# 18 — 来源增量同步桌面 Golden Path 补验

**What to build:** 关闭 17 的验收缺口。真实 Tauri 桌面应用中，首次索引后再次同步未变文件时，来源视图必须显示从 0 到总数的真实进度并正确结束；未变文件不得触发重复索引写入。

**Blocked by:** 17。

## 输入

- 17 的 `FileIndexed` 与 `BackfillCompleted` 事件语义；
- `SourcesView`、`useSources`、Tauri `sources://progress` 通道；
- 02 的来源 Golden Path 驱动与证据。

## 允许范围

- 来源 UI 的最小测试投影；
- Tauri Golden Path 驱动、测试 fixture、证据日志；
- 如发现事件到 UI 的真实接线 defect，仅允许最小修复；
- `sources.*` i18n 的必要修正；
- 18 handoff 与票据 Comments。

## 预期输出

- 可复跑的桌面 Golden Path；
- 首次索引、无变化同步、单文件变化同步三组证据；
- UI 显示 `0/N → ... → N/N`；
- 17 的状态可升级为 done 或明确列出 defect。

## 验收标准

- 首次索引后，再次同步未变文件时，真实桌面 UI 显示单调进度并收敛到 `N/N`；
- 事件 payload 与 UI 标签一致，完成事件不会把 UI 回退；
- 无变化同步后 fragment / FTS 数量不增加；
- 修改一个文件后，仅该文件重建索引，其他文件仍计入进度；
- Golden Path 有可复跑驱动、原始日志和截图；
- 不实现 03 的检索功能、04 的 watcher 或定时校验。

## 推荐测试

- `useSources` / `SourcesView` 投影测试；
- Rust `sources` 集成测试；
- 真实 `tauri dev` 或测试安装包 Golden Path。
