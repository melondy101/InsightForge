# Contributing to 织知 InsightForge

感谢你对织知的兴趣。本项目遵循测试驱动 + 垂直切片的工作流，欢迎以 PR / Issue 方式参与。

---

## 1. 开发环境

```bash
# 安装依赖
bun install

# 启动前端开发服务器（Vite）
bun run dev

# 启动桌面应用开发模式（Vite + Tauri 2 壳）
bun run tauri dev

# 运行测试
bun run test

# 代码检查
bun run lint

# 类型检查
bunx tsc --noEmit

# 前端构建（Vite -> dist/）
bun run build

# 提交前一键门禁（前端 lint/tsc/test/build + Rust fmt/clippy/test/build）
bun run gate
```

---

## 2. 分支与提交

- 功能分支：`feat/<short-name>`
- 修复分支：`fix/<short-name>`
- 提交信息遵循 [Conventional Commits](https://www.conventionalcommits.org/)：
  - `feat: add search page`
  - `fix: guard hydration mismatch on library detail`
  - `docs: update PRD with MCP tools`

---

## 3. 测试驱动开发（TDD）

1. **先写一个测试**：描述你想要的行为，确认它因正确原因失败。
2. **最小实现**：只做让测试通过的最小改动。
3. **重构**：测试全绿后清理重复、拆分职责。
4. **一次只让一个测试失败**。

测试文件与实现同目录，命名 `*.test.ts`，使用 Vitest。

```bash
# 运行全部测试
bun run test

# 监听模式
bun run test:watch
```

---

## 4. 代码规范

- **测行为，不测实现**：测试验证公共接口的可观察行为，而非内部细节。
- **纯函数优先**：核心逻辑放在 `src/lib/zhizhi/*.ts`，便于测试和复用。
- **禁止水平切片**：不要先写完所有测试再写所有实现，采用垂直切片。
- **i18n**：用户可见文案一律走 `react-i18next`，禁止硬编码。
- **交互元素**：带 `data-el="..."` 便于定位与测试。

---

## 5. 提交前三绿（前端）

在推送前，确保以下前端命令全部通过：

```bash
bun run test              # 全过
bunx tsc --noEmit         # 0 错误
bun run lint              # 0 error / 0 warning
```

---

## 6. Rust 门禁

提交 Rust 变更前，确保以下命令全部通过：

```bash
cargo fmt --check         # 格式规范
cargo clippy -- -D warnings  # 0 lint 警告
cargo test                # 全过
cargo build               # 编译通过
```

> 完整前端 + Rust 门禁可直接运行 `bun run gate`。

---

## 7. 文档同步

修改功能或添加能力时，同步更新以下文档（如适用）：

- `README.md` — 用户入口、安装/运行步骤
- `CONTEXT.md` — 领域术语、页面地图、动作定义
- `docs/PRD.md` — 产品需求与功能模块
- `docs/ARCHITECTURE.md` — 架构原则、数据流、安全设计
- `docs/ROADMAP.md` — 版本里程碑与计划
- `docs/adr/` — 重要架构决策记录（ADR）

---

## 8. 安全与合规

- 禁止提交密钥、凭证或敏感配置。
- 代转路由的 BYOK key 仅在单次请求内存中使用，绝不落盘 / 写日志。
- 新增外部依赖需评估安全影响，避免引入高危漏洞。

---

## 9. 桌面端开发

桌面端基于 Tauri 2 + Rust 本地核心。

```bash
bun run tauri dev        # 开发模式（Vite dev server + Tauri 2 壳）
bun run tauri build      # 打包桌面应用（NSIS/MSI）
```

SQLite 数据库由 Rust repository 在首次启动时于应用数据目录自动创建（`insightforge.sqlite`）并执行版本化迁移，无独立迁移命令。详见 `src-tauri/` 与 `docs/ARCHITECTURE.md`。

---

## 10. 行为准则

- 尊重反代写理念：AI 只出骨架，绝不代写成段文字。
- 保持文档与代码同步，避免知识腐烂。
- 遇到问题先查 `docs/` 和 `AGENTS.md`，再提 Issue。
