# Handoff · 14 配置修复（updater 生产就绪边界）

- 性质：主控工程 Agent 处理的 P0 应用级配置缺陷 + 14 票部分验收（无签名证书部分保持 BLOCKED）
- 基线 SHA：1c764d34372c9501ffd257bf8c20a18750b044cf
- 分支：ticket/14-config-fix
- Worktree：D:\Develop\wt\InsightForge\14-config-fix
- 角色/会话：主控 / 当前会话
- 状态：待主控合并（已全量门禁 + 真实启动验证）

## 背景（P0 阻塞）
03/07 Finalizer 在真实运行 un run tauri dev 时发现整个应用（含 dev）无法启动：tauri.conf.json 的 plugins.updater.endpoints 为 map {"default": ...}，而 tauri-plugin-updater 2.11.0 的 Config.endpoints 是 Vec<Url>（数组）→ 插件初始化 panic：invalid type: map, expected a sequence。进一步实测发现两个事实：
1. plugins.updater 必须存在且为合法 Config 对象（移除后 Tauri 注入 null → invalid type: null, expected struct Config）。
2. Tauri 2 运行时对 tauri.conf.json 中的 \ **不做替换**（设置了 env 仍为字面量）→ 占位 endpoint 导致 elative URL without a base 崩溃。原 14 方案依赖的 \ 注入机制在运行时路径不成立。

## 改动（仅 src-tauri/tauri.conf.json）
1. plugins.updater：由 map endpoints + 错放 installerName → { "pubkey": "", "endpoints": [] }（结构合法、应用可启动、无任何假公钥/假 endpoint）。
2. undle.windows.timestampUrl：http → https（P1 修复）。
3. installerName 从 plugins.updater.windows 移除（非 Tauri 2 合法字段，被静默忽略，无功能影响）。

## 生产就绪设计（不依赖 \ 运行时替换）
- pubkey：由代码注入 src-tauri/src/lib.rs::updater_pubkey()（release 缺 INSIGHTFORGE_UPDATER_PUBKEY 即 panic fail-closed；debug 回退测试公钥）。config 中 pubkey: "" 恒被 Builder.pubkey() 覆盖，为惰性占位。
- endpoint：插件 UpdaterBuilder 支持运行时 .endpoints(Vec<Url>)（tauri-plugin-updater 2.11.0 updater.rs 已确认）。真实发布 endpoint 需在实现更新检查功能时经该 API 注入。当前无真实发布基础设施 → **BLOCKED**（如实记录，不伪造）。
- 证书签名：undle.windows.certificateThumbprint 保持注入占位；无证书 → Windows 签名安装→升级实测 **BLOCKED**。build.rs 在非 debug 下强制要求 PUBKEY/ENDPOINT/THUMBPRINT 三 env，缺则 panic（fail-closed）。

## 验收标准 → 证据映射
| 验收标准 | 实现位置 | 可复跑命令 | 原始结果 | 状态 |
| --- | --- | --- | --- | --- |
| 应用可启动（P0 修复） | tauri.conf.json | un run tauri dev | 应用启动成功：database ready（applied_migrations=8）、settings ready、窗口进程运行，无 updater panic | PASS |
| 无假公钥/假 endpoint | tauri.conf.json | node 校验 + 代码审查 | updater = {pubkey:"", endpoints:[]}；无 \ 占位、无硬编码 URL | PASS |
| release 配置缺失时失败（fail-closed） | src-tauri/build.rs | cargo build --release（不设 env） | exit 101，build.rs:16 panic 列出 3 个必需 env | PASS |
| timestampUrl HTTPS | tauri.conf.json | node 校验 | https://timestamp.digicert.com | PASS |
| 真实签名安装→升级实测 | - | - | 无证书/发布设施 → BLOCKED（如实记录） | BLOCKED |

## 已运行命令与原始结果
`	ext
bun run tauri dev (无 env)             exit 0 启动成功（进程 45316，DB applied_migrations=8）
cargo build --release (无 env)          exit 101 build.rs panic：Release build requires updater env vars: [...3 env...]
cargo fmt --check                       exit 0
cargo clippy --workspace --all-targets -- -D warnings   exit 0
cargo test --workspace                  exit 0
cargo build --workspace                 exit 0
bunx tsc --noEmit                       exit 2（8 错 = 03/07 预存，本分支未引入新错）
bun run lint                            exit 1（6 错 = 03/07 预存）
bun run test                            exit 0（106）
bun run build                           exit 0
`
说明：tsc/lint 的 8/6 错属 03/07 Finalizer 范围，将在 03/07 合并后清零。

## 风险、限制
- undle.windows.certificateThumbprint 为 \ 占位：Tauri 2 运行时不做替换，但该字段仅由 CLI 打包签名时读取；签名本身 BLOCKED（无证书），build.rs 已在非 debug 下强制 env 存在。若未来 CLI 也不替换占位符，签名前需改为直接注入（届时记录）。
- endpoint 注入机制已确认（UpdaterBuilder.endpoints），待真实发布设施 + 更新检查功能时接线。
- 未发布安装包、未泄露 secret、无失败会话。

## 推荐下一步
合并至 integration 后全量回归；随后执行 03/07 原生 GUI Golden Path（应用已可启动），验证搜索→固定→引用真实用户路径后合并 03/07；再评估 15 Builder。