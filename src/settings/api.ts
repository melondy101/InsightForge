import { invoke } from '@tauri-apps/api/core';

import { toCommandError, type CommandErrorShape } from '../sources/api';

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

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    throw toCommandError(err);
  }
}

export type { CommandErrorShape };

export const settingsApi = {
  get: () => call<AiSettings>('settings_get'),
  setAi: (baseUrl: string | null, model: string | null) =>
    call<AiSettings>('settings_set_ai', { baseUrl, model }),
  setAiEnabled: (enabled: boolean) => call<AiSettings>('settings_set_ai_enabled', { enabled }),
  setApiKey: (apiKey: string) => call<AiSettings>('settings_set_api_key', { apiKey }),
  clearApiKey: () => call<AiSettings>('settings_clear_api_key'),
  testConnection: () => call<TestConnectionReport>('settings_test_connection'),
  aiSample: (prompt: string) => call<AiSampleReply>('ai_sample', { prompt }),
};
