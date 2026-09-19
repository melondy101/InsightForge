# ADR 0002：新增远端同步服务作为第三类 runtime

- 状态：已接受
- 日期：2026-09-18
- 取代：[ADR 0001](./0001-byok-over-eazo-app-ai.md)（已标注 superseded）
- 修订：`docs/PRD.md` §5.2 第一条「不做 Web 版、服务端 API」、`CONTEXT.md` 铁律 3 与铁律 4、`docs/ARCHITECTURE.md` §1 原则表
- 保留：`CONTEXT.md` 铁律 1、2、5 全文不变

## 背景

织知首版是纯桌面应用（Windows + Tauri 2 + Rust core + SQLite），`PRD.md:62` 与 `CONTEXT.md:122`
明确写下「不做 Web 版、服务端 API、Postgres、MCP；旧 Web 版全部移除」。

现在的目标变成：同一份产品要能本地下载安装包，也能作为网页部署，且两端数据互通；
发布后网页与桌面都能自动更新；安装包由 GitHub Actions 自动编译。

「网页端 + 数据互通」无法在纯桌面架构下达成：浏览器拿不到用户本地的 SQLite 与授权目录。
四条候选路径中（本机引擎 + 托管静态站 / 远端同步服务 / 只读发布站 / 纯静态 PWA + 本地导入），
本项目选择**远端同步服务**：服务器持有作者资产的副本并在设备间同步，网页端可独立登录使用。

本 ADR 记录这个选择，以及为了让它成立而对五条铁律做的**最小**修订。

## 决定

### 1. 三类 runtime，一份核心

| runtime | 形态 | 承担 |
| --- | --- | --- |
| 桌面端 | Windows 安装包（Tauri 2） | 知识库引擎：目录授权、扫描、索引、FTS 检索、磁盘 `.md` 原子保存、BYOK 出站 |
| 服务端 | `insightforge-server`（Rust + axum，复用 `insightforge-core` 领域规则） | 作者资产副本存储、设备注册、同步与冲突判定、静态站托管 |
| 网页端 | 同一份 React 代码构建出的静态站 | 通过 `CommandClient` 的 HTTP adapter 访问服务端；不直接接触 SQL 或文件系统 |

前端只依赖一个 seam（`CommandClient`），桌面装配 Tauri adapter，网页装配 HTTP adapter。
`crates/core` 不引入 `tauri` 依赖，服务端与桌面端共享同一份领域实现。

### 2. 同步范围：只同步「非派生的作者资产」

- **同步**：草稿正文与元数据、思考卡、引用快照、陪练会话与轮次、AI 调用审计。
- **不同步**：来源的本机绝对路径、`source_file.file_identity`、片段与 FTS 表、扫描游标与缓存。
  这些仍是派生数据或本机标识——每台设备对自己授权到的目录各自重建索引（铁律 5 不变）。

### 3. 互通契约取代「本机路径即身份」（本 ADR 的最大代价）

今天 `knowledge_source.root_path`（绝对路径）、`draft.path` 是 UNIQUE 键，
`file_identity` 形如 `win:<vol>:<idx>`。两台设备会把同一个知识库判成两个。

因此跨设备身份一律改用不透明逻辑 ID：`knowledge_source.id` / `draft.id` 等主键已是
UUIDv4，源文件级身份是 `(source_id, rel_path)`——`rel_path` 本来就是相对路径，
所以 `file_identity` 继续留在设备本地，不需要再造一个 `file_key` 映射表；
每条同步记录带 `rev`（**写入设备本地单调递增**，落库时由存储层触发器打点，
而非服务端分配——离线时也必须能定序）与 `origin_device`（最后一次写入它的设备），
服务端只保管每条逻辑身份所见过的最高修订；跨设备判序比较 `(rev, origin_device)`，
同修订不同内容即真冲突。
时间戳由单一 `Clock` 产生（当前有 5 份精度不同的 `now_iso()`）。

两条本地 UNIQUE 约束（`knowledge_source.root_path`、`draft.path`）**保留**：
它们表达的是「同一台设备不能授权同一目录两次」「同一目录不可能有两个同名文件」，
本就是设备本地的不变量，也是 `sources.add` 拒绝重复根目录的依据。
跨设备身份一律由 `id` 承担（`identity::new_id()` 铸造 UUIDv4），同步载荷永不含路径。

### 4. 铁律 4 修订：磁盘 `.md` 仍是**渲染真相**，服务器副本是**同步真相**

原文：「磁盘上的 `.md` 是唯一真相」。
修订为：桌面端上，磁盘 `.md` 仍是该文件的唯一渲染真相，保存基线与原子替换协议一字不改；
服务器上存在一份用于跨设备同步的副本，它不是编辑目标，只在冲突时按 `rev` 参与合并。

后果（明确接受）：副本与磁盘可能短暂分叉；离线编辑后重连可能产生真冲突而非「覆盖」。
因此网页端的写入必须走同一套 `Conflict` 三选一协议，绝不静默覆盖。

### 5. 铁律 3 修订：两类出站包，API Key 仍永不出本机

原文：「仅 `EgressBundle` 可出站」。
修订为：新增第二类出站包 `SyncBundle`（同步载荷，含草稿正文与作者资产），
与 `EgressBundle` 同等对待——生成即明示数量与字段，写入同一份 `ai_call_audit` 审计。

边界不动的两点：

- **API Key 不进服务器**：服务端不持有、不代理任何 AI 凭据。
- **网页端要触发陪练时**，请求进服务端队列，由该用户的桌面端领取、在本地用本地 Key 执行、
  回写结果。桌面端离线时，网页端的 AI 能力显式关闭并说明原因，不做「服务器代填 Key」。

模型调用仍受五类动作白名单约束（铁律 2 不变），疑似连续正文照旧丢弃。

> 实现状态（2026-09-19）：`SyncBundle` 已落在 `crates/core/src/egress.rs`——
> 白名单 `SYNCED_ASSETS` 逐表声明可出站的列，`DEVICE_LOCAL_COLUMNS`
> （`path`/`root_path`/`abs_path`/`file_identity`/`baseline_*`）为黑名单，
> `build` 命中即返回 `DeviceLocalColumn` 错误；`declared()` 给出「每张表几条、哪些字段」；
> `AuditLogger::log_sync` 写入同一份 `ai_call_audit` 且 `kind = 'sync'`（迁移 0018），
> 出站审计界面按 kind 区分显示。约束由 `tests/sync_bundle_contract.rs` 5 条用例守住。
> 尚缺的是**发送方**：服务端与 `sync.push/pull` 命令还没实现，所以今天没有任何东西真的出站。
>
> 同一批工作把设备身份的铸造从「桌面启动时调一次」下沉到 `Repository::open`：
> 触发器盖 `origin_device` 时若还没有 `device.id`，那一行会永久留下 NULL，
> 而这台设备对这些行的写序就再也参与不了判定——存储层既然负责打点，就得负责先有身份。

### 6. 分发与更新

- 桌面：GitHub Actions 打 Windows 安装包（MSI/EXE），产物上传 GitHub Release，
  `latest.json` 由同一 pipeline 生成。签名公钥是公开数据、随 `tauri.conf.json` 进仓；
  私钥才是 CI secret。endpoint 由构建期注入（`tauri.conf.json` 里保留 dev 占位值，
  release 构建缺真实值时 `build.rs` 直接 panic）。
- 网页与服务端：同一 tag 触发部署 job，「部署即更新」，无独立版本通道。
  现状：桌面侧 `release.yml` 已落地（tag → 打包 → minisign 签名 → Release + `latest.json`），
  签名公钥已换成真实 minisign 公钥，endpoint 由 CI 注入；
  网页侧部署 job **刻意尚未添加**——前端还没有 HTTP adapter，此刻部署等于发布一个不能用的壳。
- 平台矩阵本期只承诺 Windows（与 `PRD.md:59` 一致）；Linux/macOS 不做承诺，
  `secrets.rs` 在非 Windows 拒绝服务的路径保持不变。

### 7. 「一份核心」能跑在哪些 runtime：先探边界，不预先分层

2026-09-19 用编译器而不是推测来回答这个问题，两条探测都在本机实跑：

- **Linux（`rust:1.90.0` 容器，`x86_64-unknown-linux-gnu`）**：`cargo test -p insightforge-core --locked`
  与 `cargo clippy -p insightforge-core --all-targets -- -D warnings` 全绿。探测前有三处不兼容，
  都是这次修掉的：`tests/sources.rs` 未加 cfg 就调用 `std::os::windows::fs::symlink_file`
  （改成平台分派 helper，同一逃逸断言在 Linux 上真的跑通了）；junction 测试用的是 Windows 专有机制
  （`#[cfg(windows)]`）；`scan.rs`/`secrets.rs` 各有一处非 Windows 下的死代码警告。
  注意 `ci.yml` 的 ubuntu 腿早就配了 clippy `-D warnings` + `cargo test`，但从未在本地验证过——
  在这三条 commit 之前，那条腿必然是红的。**服务端复用 core 的前提由这条 CI 机械保证，不需要新的抽象层。**
- **wasm32-unknown-unknown**：`cargo check` 死在依赖图第一层（`getrandom` 要求 `js` feature，经 `uuid`
  传进来），还没轮到本仓库的代码。要在浏览器里跑 core，得同时换掉 `rusqlite`(bundled)、
  阻塞式 `ureq`、`std::thread` 执行器，并给 `getrandom` 装 `js`——那是换一套依赖，不是加一层 port。
  而本 ADR 的网页形态（§1）本来就是静态站 + HTTP adapter，不含「浏览器里跑 SQL/文件」。

**决定**：不给 `crates/core` 预置 ports & adapters 分层。把 85 条 SQL 逐表换成 port 通不过 deletion test——
复杂度只是搬家。重新评估的触发条件写清楚：真要出 Linux/macOS 桌面包时，`secrets.rs` 需要第 3 个
SecretStore adapter（keyring 后端），那是有人要用的真实需求，届时再补。

## 代价与风险

1. **重新引入运维面**：账号体系、传输加密、服务端数据存储与删除策略、备份。旧 Web 版正是带着这些被移除的。
2. **隐私面扩大**：草稿正文首次离开用户机器。需在首次同步前明示，并提供「只同步元数据」的关闭项。
3. **合并语义**：本地保存基线协议解决的是「同一台机器上两个编辑器」，跨设备冲突是新增的一类问题，
   需要真实现（`rev` + 三选一），不是既有代码的自然延伸。
4. **`ARCHITECTURE.md:170`** 写明「纯浏览器 renderer 模式不能代替真实文件与安装测试」——该判断仍然成立，
   因此索引/检索/落盘路径的测试仍必须在桌面端做，网页端 e2e 只覆盖同步与阅读路径。
5. **工作量**：`ROADMAP.md` 的既有阶段排期需要为重排让路。

## 后续（本 ADR 不决定）

- 服务端数据库选型（SQLite per-tenant vs Postgres）与托管形态。
- 账号体系（自带 vs 第三方登录）与免费额度。
- 端到端加密同步：若采纳，可把风险 2 收回大半，代价是服务端失去检索能力。
