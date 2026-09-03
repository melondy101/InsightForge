# InsightForge Windows E2E

`e2e/golden-path.spec.ts` 用真实 `data-el` 选择器与断言覆盖 7 条黄金主路径（首次启动→索引→中文检索、证据→草稿→引用、编辑→保存→外部冲突三选一、删来源保留快照、导出、无 AI Key 降级、重启状态一致）。

## 门控（BLOCKED，不伪 PASSED）

规格整体受探针门控，绝不在无 WebView2 时假绿：

- 仅当 `INSIGHTFORGE_E2E_RUN=1` **且**本机已安装 WebView2（注册表 `HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A08C11}` 存在）时，用例才运行；
- 否则 `test.skip(!probe.ok, reason)` 整体 BLOCKED，由 CI/本机明确报告为“未运行”，不计入通过。

探针实现见 `e2e/support/probe.ts`；隔离的临时数据目录由 `e2e/support/fixtures.ts` 注入 `INSIGHTFORGE_E2E_DATA_DIR` 并自动清理。

## 前置条件（真实验收）

1. Windows 10/11 已安装 WebView2 Runtime。
2. 真实签名安装包、可访问的 HTTPS updater endpoint，以及安全注入的 updater 私钥（仅公开发布需要）。
3. 设置 `INSIGHTFORGE_E2E_RUN=1`。

```powershell
$env:INSIGHTFORGE_E2E_RUN = '1'
bun run test:e2e
```

## 手动兜底

`e2e/manual/verify.ps1` 是 Windows 手动验证清单（来源索引→中文检索、证据→引用、冲突三选一、删除来源保留快照、导出、无 AI Key 降级、重启状态保持、诊断脱敏）。它不依赖 Playwright，适合在真实安装包上逐条核对。

个人自用不要求购买代码签名证书；但这不构成公开发布签名或安装升级验证通过的证据。
