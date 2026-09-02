# AGENTS.md · 织知 InsightForge 贡献与测试规范

> 本文件固化本项目的开发方法论，供人类与 AI 协作者共同遵循。
> 核心方法论借鉴 `tdd-workflow`（测试驱动、垂直切片），并结合织知既有实践。

## 一、核心原则：测试驱动 + 垂直切片（曳光弹）

- **测行为，不测实现**：测试验证的是「通过公共接口能观察到的行为」，而非内部实现细节。一次彻底的内部重写，好的测试应当依然全绿。
- **禁止水平切片**：不要「先写完所有测试，再写所有实现」——那会写出「想象中的行为」，一碰现实就崩。
- **采用垂直切片**：`写一个测试 → 用最小实现让它通过 → 重复`。每一轮都用上一轮的真实认知。
- **任何时候不超过一个失败的测试**。

## 二、标准工作流（每个功能）

1. **规划**：先确认公共接口（函数签名 / props / 路由），列出按优先级排序的行为，明确每条「做完」的标准。此阶段不写代码。
2. **曳光弹**：写一个描述首个行为的测试 → 确认它因正确原因失败 → 写最小实现 → 确认变绿。
3. **增量循环**：按优先级对每个行为重复第 2 步；出现明显重复时做微重构。
4. **重构**：全部行为覆盖后，抽取重复、拆分职责过重的函数；重跑测试须保持全绿。若某次重构弄坏了测试，说明该测试耦合了实现——修测试。

## 三、测试质量清单（标记测试完成前逐条核对）

| 检查 | 通过条件 |
|------|---------|
| 公共接口 | 测试调用对外 API，而非内部函数 |
| 行为断言 | 断言「系统做了什么」，而非「怎么做的」 |
| 抗重构 | 完整重写内部实现后，测试仍绿 |
| 像规格说明 | 不熟悉代码的人也能读懂测试意图 |
| 单一行为 | 每个测试只断言一个行为 |

## 四、织知的落地约定（本项目特有）

- **纯函数优先**：核心逻辑（索引、搜索、草稿、陪练、导出格式化、度量等）实现为 `crates/core/src/*.rs` 的纯函数，注入 `now` 等外部依赖以便测试与 SSR 一致。
- **测试框架**：前端用 `vitest`，测试文件与实现同目录、命名 `*.test.ts`，运行 `bun run test`；Rust 用 `cargo test`。
- **提交前三绿**：前端 `bun run test` 全过、`bunx tsc --noEmit` 0 错误、`bun run lint` 0 error/warning；Rust `cargo test` 全过、`cargo build` 通过。
- **逐项交付**：一个功能一条 commit，本地跑通验证后单独推送到远端。
- **反代写定位**：新功能应服务于「AI 只出骨架/提示、血肉由创作者写」。检查/提示类能力（只提示不改写）优先于替写类。
- **i18n**：用户可见文案一律走 `react-i18next` 的 `t()`，字符串存 `src/i18n/locales/{zh-CN,en-US}.json`，不硬编码。

## 五、Tauri 命令清单（只注册业务命令）

只注册窄而有类型的业务命令，不放通用能力。前端薄投影在 `src/sources/api.ts` 等文件。

```
sources.add / list / pause / remove / sync / reconcile / rebuild_index / set_sync_interval
search.query / open_context
sources_await_index_committed   # 阻塞等待索引提交完成，返回 IndexStatus（含 searchable 布尔）
drafts.open / save / resolve_conflict
coach.start / respond / resume / mark_helpful
thoughts.confirm / update
references.create / list / adopt / mark_source_deleted
egress.build / audit_log / audit_list
settings.get / set_ai / set_ai_enabled / set_api_key / clear_api_key / test_connection
```

## 六、发布构建

- 前端：`bun run build`（Vite 构建到 `src-tauri/dist`）。
- 桌面版：`bun run tauri dev`（开发模式）；`bun run tauri build`（打包桌面应用）。
- 提交前门禁：前端 `bun run test` / `bun run lint` / `bunx tsc --noEmit`；Rust `cargo test` / `cargo build` / `cargo fmt --check` / `cargo clippy -- -D warnings`。
