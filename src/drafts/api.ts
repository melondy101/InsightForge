import { callCommand } from '../native/command-client';

/**
 * drafts 命令 seam 的前端薄投影（ARCHITECTURE §4）。
 * 类型与 crates/core/src/drafts.rs 的 serde 输出一一对应。
 */

export interface DraftFile {
  id: string;
  path: string;
  filename: string;
  baselineHash: string;
  baselineMtimeMs: number;
  baselineSize: number;
  createdAt: string;
  updatedAt: string;
}

export interface Conflict {
  message: string;
  baselineContent: string;
  localContent: string;
  externalContent: string;
}

/**
 * Rust 将 Conflict 序列化为 `DraftsError::Conflict(message)` 中的 message 字段
 * （camelCase JSON）。前端需完整解析，尤其是 externalContent——否则「采用外部」
 * 将丢失可用内容。解析失败时返回 null，由调用方回退到原始 message。
 */
const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null;

export function parseConflict(message: string): Conflict | null {
  if (!message) return null;
  try {
    const value = JSON.parse(message);
    if (
      isRecord(value) &&
      typeof value.message === 'string' &&
      typeof value.localContent === 'string' &&
      typeof value.externalContent === 'string'
    ) {
      return {
        message: value.message,
        baselineContent:
          typeof value.baselineContent === 'string' ? value.baselineContent : '',
        localContent: value.localContent,
        externalContent: value.externalContent,
      };
    }
    return null;
  } catch {
    return null;
  }
}

export const draftsApi = {
  ensureDirectory: () => callCommand<void>('drafts_ensure_directory'),
  list: () => callCommand<DraftFile[]>('drafts_list'),
  create: (filename: string) => callCommand<DraftFile>('drafts_create', { filename }),
  open: (filename: string) => callCommand<DraftFile>('drafts_open', { filename }),
  save: (filename: string, content: string) =>
    callCommand<DraftFile>('drafts_save', { filename, content }),
  delete: (filename: string) => callCommand<void>('drafts_delete', { filename }),
  resolveConflict: (filename: string, resolution: 'keep_local' | 'adopt_external' | 'save_as_copy', localContent: string) =>
    callCommand<DraftFile>('drafts_resolve_conflict', { filename, resolution, localContent }),
};
