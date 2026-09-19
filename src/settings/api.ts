import { callCommand } from '../native/command-client';

/**
 * settings 命令 seam 的前端薄投影（08 号票）。
 * 类型与 crates/core/src/settings.rs 的 serde 输出一一对应。
 */

export interface AiSettings {
  baseUrl: string | null;
  model: string | null;
  /** Key 是否已配置；明文永不离开系统凭据库。 */
  apiKeyConfigured: boolean;
  enabled: boolean;
}

export interface TestConnectionReport {
  ok: boolean;
  httpStatus: number | null;
  /** 已脱敏的诊断信息（不含 Key 明文）。 */
  detail: string | null;
}

export interface AiSampleReply {
  model: string;
  content: string;
}

export const settingsApi = {
  get: () => callCommand<AiSettings>('settings_get'),
  setAi: (baseUrl: string | null, model: string | null) =>
    callCommand<AiSettings>('settings_set_ai', { baseUrl, model }),
  setAiEnabled: (enabled: boolean) =>
    callCommand<AiSettings>('settings_set_ai_enabled', { enabled }),
  setApiKey: (apiKey: string) => callCommand<AiSettings>('settings_set_api_key', { apiKey }),
  clearApiKey: () => callCommand<AiSettings>('settings_clear_api_key'),
  testConnection: () => callCommand<TestConnectionReport>('settings_test_connection'),
  aiSample: (prompt: string) => callCommand<AiSampleReply>('ai_sample', { prompt }),
};
