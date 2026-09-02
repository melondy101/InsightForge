# ADR 0001：BYOK 而非 Eazo App AI

- 状态：已被桌面版架构取代
- 原始日期：2026-08-09
- 取代日期：2026-09-02

此 ADR 描述的是已冻结 Web 版的 Next.js、Postgres、服务端 API 路由和 Eazo 对比，相关代码已不在当前仓库。

当前决策见 [ARCHITECTURE.md](../ARCHITECTURE.md) §7：桌面版使用 Rust AI 网关和 OpenAI 兼容 BYOK；密钥存 OS 凭据库，只有 `EgressBundle` 可出站。没有 Web API 路由、`.env` 配置或 Eazo App AI 集成。
