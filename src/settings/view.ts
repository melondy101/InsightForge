import type { AiSettings, TestConnectionReport } from './api';
import type { CommandErrorShape } from '../sources/api';

/**
 * 设置视图的状态投影（纯函数，可 vitest 直测）：
 * 前端只是命令输出的薄投影（spec §Testing Decisions）。
 */

/** settings.error.* 下的已知错误码（与 Rust SettingsError::code() 一一对应）。 */
const KNOWN_ERROR_CODES = [
  'ai_disabled',
  'ai_no_base_url',
  'ai_no_model',
  'ai_no_api_key',
  'invalid_base_url',
  'invalid_api_key',
  'ai_http_status',
  'invalid_response',
  'secret_store_error',
  'repository_error',
  'client_error',
] as const;

export interface ApiKeyView {
  /** 已配置徽标（描述凭据库里的状态；输入框永不回显明文）。 */
  badgeVisible: boolean;
  clearVisible: boolean;
  canSaveKey: boolean;
  placeholderKey: 'settings.apiKeyPlaceholder' | 'settings.apiKeyPlaceholderConfigured';
}

/**
 * Key 区域状态：配置态（badge/清除按钮）与草稿态（能否保存）是两个独立维度——
 * 已配置时输入框只是「重录入口」，不回显也不表示清空。
 */
export function apiKeyView(apiKeyConfigured: boolean, draft: string): ApiKeyView {
  return {
    badgeVisible: apiKeyConfigured,
    clearVisible: apiKeyConfigured,
    canSaveKey: draft.trim().length > 0,
    placeholderKey: apiKeyConfigured
      ? 'settings.apiKeyPlaceholderConfigured'
      : 'settings.apiKeyPlaceholder',
  };
}

export interface AiStatusView {
  key: 'settings.aiStatus.on' | 'settings.aiStatus.off' | 'settings.aiStatus.incomplete';
}

/** 总开关状态渲染：关闭优先；开启但缺任一配置 = 配置不完整。 */
export function aiStatusView(
  ai: Pick<AiSettings, 'enabled' | 'baseUrl' | 'model' | 'apiKeyConfigured'>,
): AiStatusView {
  if (!ai.enabled) {
    return { key: 'settings.aiStatus.off' };
  }
  const complete = ai.baseUrl !== null && ai.model !== null && ai.apiKeyConfigured;
  return { key: complete ? 'settings.aiStatus.on' : 'settings.aiStatus.incomplete' };
}

export interface TestResultView {
  tone: 'ok' | 'fail';
  key: string;
  params?: Record<string, unknown>;
}

/** 连接测试报告 → 展示投影（诊断信息已在 Rust 侧脱敏）。 */
export function testResultView(report: TestConnectionReport): TestResultView {
  if (report.ok) {
    return { tone: 'ok', key: 'settings.testResult.ok' };
  }
  if (report.httpStatus !== null) {
    return {
      tone: 'fail',
      key: 'settings.testResult.failStatus',
      params: { status: report.httpStatus },
    };
  }
  return {
    tone: 'fail',
    key: 'settings.testResult.failTransport',
    params: { message: report.detail ?? '' },
  };
}

export interface ErrorView {
  key: string;
  params?: Record<string, unknown>;
}

/** 命令错误 → i18n 键；未知 code 回退 generic（带 Rust message 供诊断）。 */
export function settingsErrorView(error: CommandErrorShape): ErrorView {
  if ((KNOWN_ERROR_CODES as readonly string[]).includes(error.code)) {
    return { key: `settings.error.${error.code}` };
  }
  return { key: 'settings.error.generic', params: { message: error.message } };
}
