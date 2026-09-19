# 安全说明 / Security Notes

本文件记录织知 InsightForge 的安全审查基线与**明确接受**的残余风险，便于审计留痕。

> 基线对应**桌面版 + [ADR 0002](./docs/adr/0002-web-remote-sync-service.md) 的客户端侧**（2026-09-19 重写）。
> 旧 Web 版（Next.js / Postgres / drizzle / 三条 LLM 代转路由 / localStorage 存 key）已冻结，
> 那些代码与依赖在当前仓库里不存在，其审查结论不再冒充现状；差异见 `docs/release-readiness.md` §5 D5。
> 表中每条都给了可复跑依据——写不出依据的一律不标 ✅。

## 当前基线

| 维度 | 状态 | 依据 |
|---|---|---|
| 凭据存放 | ✅ API Key 只进 OS 凭据库（Windows Credential Manager），非 Windows 平台明确拒绝服务；不入 SQLite、不入日志、不入任何出站包、服务端不持有不代理 | `crates/core/src/secrets.rs` + `cargo test -p insightforge-core --test secrets` |
| renderer 权限面 | ✅ 主窗口只有 `core:default` + `dialog:default`；前端仅能调 50 条窄而有类型的业务命令，无任意 FS / 网络 / SQL 能力 | `src-tauri/capabilities/default.json`、命令清单见 `docs/ARCHITECTURE.md` §4 |
| HTML 注入面 | ✅ 已修（2026-09-19）：检索证据卡的片段此前经 `dangerouslySetInnerHTML` 渲染，知识库文件里的 `<img onerror=…>` 会在 renderer 里执行；现在按 `<mark>` 标记切分后一律当文本渲染，并有一条全仓守门测试禁止再出现 `dangerouslySetInnerHTML`。导出 HTML 走 `rehype-sanitize` + 标签/属性白名单，内嵌 Markdown HTML 默认禁用 | `src/search/snippet.ts` + `src/search/snippet.test.ts`、`src/export/export.ts` |
| SQL 注入面 | ✅ 用户可控的值一律走绑定参数（`rusqlite::params!` / `params_from_iter`）。拼进 SQL 的只有编译期常量：表名与列名来自 `SYNCED_ASSETS` / `SOURCE_COLUMNS` 常量（且有守卫拒绝设备本地列混进白名单）、`NOT IN (?,?,…)` 里拼的是占位符个数而非路径 | `crates/core/src/{store,repository/mod,egress}.rs`，`egress.rs` 的守卫由 `tests/sync_bundle_contract.rs` 拦下 |
| 出站最小化 | ✅ 只有 `EgressBundle`（AI）与 `SyncBundle`（同步）两类载荷；同步载荷的可出站列是逐表白名单，`path`/`root_path`/`abs_path`/`file_identity`/`baseline_*` 在黑名单里，构建命中即报错；发送前明示数量 | `crates/core/src/egress.rs` + `tests/sync_bundle_contract.rs`（5 条） |
| 知识库只读 | ✅ 源文件写入在 Rust 侧被拒绝；symlink / junction 逃逸出根目录被拒且源文件 hash 不变 | `cargo test -p insightforge-core --test sources` |
| 派生数据可重建 | ✅ 删索引库不丢作者资产（引用快照、思考卡、保存基线、陪练轮次不是缓存） | `tests/schema.rs`、`tests/document_store_contract.rs` |
| 诊断产物脱敏 | ✅ 快照只含版本/平台/SQLite/迁移/完整性/表计数；事件日志字段级替换禁记清单命中值；两者都不含 Key、正文与本机路径 | `crates/core/src/diagnostics.rs` + `--test` 内联用例 |
| 错误信息 | ⚠️ `CommandError.message` 会透出底层字符串（可能含本机路径）。仅应用内对本人可见，不进诊断、不进审计；接入任何遥测前必须先过滤（`docs/HANDOFF.md` R4） | `src/ui/error-text.ts` |
| 静默覆盖 | ✅ 本机与跨设备冲突都走三选一，绝不静默覆盖；设备撤销不可逆 | `crates/core/src/drafts.rs`、`tests/devices_contract.rs` |

## 明确接受的残余风险 / 待办

1. **服务端未实现**：当前没有任何同步载荷真的出站。`insightforge-server` 上线前必须补齐 ADR 0002 风险 1–2
   列出的传输加密、账号体系、服务端数据删除策略，以及**首次同步前的明示 + 「只同步元数据」关闭项**。
   这一条是本文件里最大的一块空白，不要用「已实现 SyncBundle」误读为「同步已安全」。
2. **安装包未签名**（Runbook C1，可选）：首次下载有 SmartScreen 提示；updater 的 minisign 校验独立生效，
   不受影响。前提是把 updater 私钥放进 CI secret（C3，目前只在构建机 temp 目录）。
3. **依赖审计没有自动化**：CI 不含 `npm audit` / `cargo deny` 一类步骤，供应链面靠人工。
   要补的话是 ci.yml 一个步骤的事。
4. **Windows 桌面 E2E 未实测**（C4）：无 WebView2 的环境里黄金路径明确 BLOCKED，不伪造 PASSED。
5. **授权目录即隐私边界**：目录内的内容会进索引，并可最多 3 条片段进入 AI 出站包。
   「发送前明示数量」是唯一的人工闸门；改这条必须连带改契约测试与文案。
6. **网页端 AI 依赖桌面在线**：桌面离线时网页的 AI 能力显式关闭并说明原因——这是设计（不代填 Key），不是缺陷。
