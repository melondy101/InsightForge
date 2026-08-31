import { invoke } from '@tauri-apps/api/core';

/**
 * drafts 命令 seam 的前端薄投影（ARCHITECTURE §4）。
 * 类型与 crates/core/src/drafts.rs 的 serde 输出一一对应。
 */

export type DraftsErrorShape = {
  code: string;
  message: string;
};

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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

export function toDraftsError(err: unknown): DraftsErrorShape {
  if (isRecord(err) && typeof err.code === 'string' && typeof err.message === 'string') {
    return { code: err.code, message: err.message };
  }
  return { code: 'unknown', message: err instanceof Error ? err.message : String(err) };
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    throw toDraftsError(err);
  }
}

export const draftsApi = {
  ensureDirectory: () => call<void>('drafts_ensure_directory'),
  list: () => call<DraftFile[]>('drafts_list'),
  create: (filename: string) => call<DraftFile>('drafts_create', { filename }),
  open: (filename: string) => call<DraftFile>('drafts_open', { filename }),
  save: (filename: string, content: string) =>
    call<DraftFile>('drafts_save', { filename, content }),
  delete: (filename: string) => call<void>('drafts_delete', { filename }),
};
