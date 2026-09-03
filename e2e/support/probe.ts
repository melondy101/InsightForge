import { execSync } from 'node:child_process';

/**
 * E2E 可用性探针：决定整组 E2E 是否可运行。
 *
 * 关键约束（Phase D）：无 WebView2 / CDP 时，E2E 必须显式 BLOCKED（skip），
 * 绝不允许伪造 PASS。只有显式开启 `INSIGHTFORGE_E2E_RUN=1` 且本机确实存在
 * WebView2 运行时，才会真正跑黄金路径。
 */

export interface ProbeResult {
  ok: boolean;
  reason: string;
}

const WEBVIEW2_REG_KEY =
  'HKLM\\SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A08C11}';

/** 仅当 Windows 且注册表存在 WebView2 运行时 GUID 时视为可用。 */
export function isWebView2Available(): boolean {
  if (process.platform !== 'win32') {
    return false;
  }
  try {
    execSync(`reg query "${WEBVIEW2_REG_KEY}"`, { stdio: 'ignore' });
    return true;
  } catch {
    return false;
  }
}

export function probeE2E(): ProbeResult {
  if (process.env.INSIGHTFORGE_E2E_RUN !== '1') {
    return {
      ok: false,
      reason:
        'BLOCKED: E2E disabled. Set INSIGHTFORGE_E2E_RUN=1 on a host with the WebView2 runtime to run golden paths.',
    };
  }
  if (!isWebView2Available()) {
    return {
      ok: false,
      reason: 'BLOCKED: WebView2 runtime not detected on this host.',
    };
  }
  return { ok: true, reason: 'ok' };
}

/**
 * 把探针结果转成 Playwright 的 skip 原因。文件级 `test.skip(!probe.ok, reason)`
 * 会让整组测试在报告中呈现为 skipped（BLOCKED），而不是 passing 或 failing。
 */
export function skipReason(probe: ProbeResult): string {
  return probe.ok ? '' : probe.reason;
}
