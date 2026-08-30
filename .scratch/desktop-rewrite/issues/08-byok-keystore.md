# 08 — BYOK 凭据库与 AI 总开关

**What to build:** 用户在设置中配置 OpenAI 兼容的 base / model 与 API Key，Key 存入系统凭据库而非数据库或前端；AI 有总开关，关闭后除 AI 外全功能可用。AI 接入继续遵循 ADR 0001 的 BYOK 决策。

**Blocked by:** 01 — 桌面工作区骨架与 SQLite 地基

**Status:** ready-for-agent

- [ ] Key 经 `keyring` 适配层存入 Windows Credential Manager；数据库与前端拿不到明文（Tauri Stronghold 不混用）
- [ ] base / model 可配置，连接测试可用
- [ ] AI 总开关关闭时，所有 AI 命令返回明确的不可用状态，其余功能不受影响
