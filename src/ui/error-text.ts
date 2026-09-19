import { i18n } from '../i18n';
import type { CommandErrorShape } from '../native/command-client';

export type Translate = (key: string, options?: Record<string, unknown>) => string;

/**
 * Rust 侧所有 `code()` 的并集，外加命令通道自己会造的码。
 * 这里是唯一一份清单：新增错误码时补进 `errors.*`，
 * `src/ui/error-text.test.ts` 会因为缺键而红——用户不该看到 `io_error` 这种码。
 */
export const ERROR_CODES = [
  // 公共基础
  'repository_error',
  'io_error',
  'client_error',
  'secret_store_error',
  'cancelled',
  'egress_error',
  // 命令通道（桌面桥未接入 / 网页 HTTP 通道）
  'runtime_unconfigured',
  'unknown_command',
  'network_unreachable',
  'timeout',
  'http_error',
  'bad_request',
  'unauthorized',
  'forbidden',
  'conflict',
  'rate_limited',
  // AI 守门与出站
  'ai_disabled',
  'ai_no_base_url',
  'ai_no_model',
  'ai_no_api_key',
  'invalid_api_key',
  'invalid_base_url',
  'ai_http_status',
  'invalid_response',
  // 来源
  'invalid_name',
  'duplicate_root',
  'source_not_found',
  'source_paused',
  'already_indexing',
  'invalid_rel_path',
  'invalid_sync_interval',
  'invalid_root',
  'root_not_a_directory',
  'invalid_ignore_glob',
  // 草稿与文档写入
  'not_found',
  'invalid_resolution',
  'clipboard_write_failed',
  'clipboard_unavailable',
  // 引用
  'draft_not_found',
  'reference_not_found',
  'invalid_reference',
  // 检索
  'invalid_query',
  // 陪练
  'invalid_action',
  'max_rounds_reached',
  'session_not_found',
  'session_not_active',
  // 设备
  'device_revoked',
  'device_not_registered',
  'invalid_device_id',
  // 思考卡与诊断
  'thoughts_error',
  'event_log_locked',
] as const;

/**
 * 命令错误 → 用户能读懂的一句话。
 *
 * 查找顺序：视图命名空间的专属文案 → 公共 `errors.*` 表 → 公共兜底（带 Rust 原文）。
 * 专属文案优先是因为同一码在不同视图里的下一步不一样（例如 `ai_disabled` 在陪练里
 * 提示去设置，在设置里就是那个开关本身）。
 */
export function errorText(t: Translate, error: CommandErrorShape, namespace?: string): string {
  const candidates = namespace
    ? [`${namespace}.error.${error.code}`, `errors.${error.code}`, `${namespace}.error.generic`]
    : [`errors.${error.code}`];
  for (const key of candidates) {
    if (i18n.exists(key)) return t(key, { message: error.message });
  }
  return t('errors.generic', { message: error.message });
}
