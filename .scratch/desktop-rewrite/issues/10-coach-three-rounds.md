# 10 — 陪练三轮协议

**What to build:** 卡住的作者显式说「我卡住了」后，陪练给出仅限五类动作的回应，最多三轮即把笔还给作者；疑似连续正文的输出被直接丢弃并替换为本地安全提示。草稿标记发布后，陪练轮次默认折叠且不进入检索。

**Blocked by:** 09 — 出站包 EgressBundle 与调用审计

**Status:** ready-for-agent

- [ ] `coach.start` 仅由显式动作触发；`respond` / `resume` 受三轮协议与接续条件约束，三轮后终止
- [ ] 响应必须解析为白名单动作 `question | evidence | tension | directions | micro_task`（决策类型来自调研文档）；疑似连续正文丢弃 + 本地安全提示
- [ ] 轮次写入 `coach_session` / `coach_turn`；`mark_helpful` 反馈可用
- [ ] 草稿标记发布后，陪练轮次默认折叠且不进入检索结果
- [ ] LLM / HTTP 依赖在命令 seam 后注入假实现完成测试；真实链路经 09 号票据的网关出站
