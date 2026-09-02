import { invoke } from '@tauri-apps/api/core';

/**
 * references 命令 seam 的前端薄投影（ARCHITECTURE §4）。
 * 类型与 crates/core/src/references.rs 的 serde 输出一一对应。
 */

export type ReferencesErrorShape = {
  code: string;
  message: string;
};

export interface Reference {
  id: string;
  draftId: string;
  fragmentId: number;
  sourceId: string;
  sourceName: string;
  relPath: string;
  headingPath: string;
  snippet: string;
  fullText: string;
  startLine: number;
  endLine: number;
  adopted: boolean;
  sourceDeleted: boolean;
  createdAt: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

export function toReferencesError(err: unknown): ReferencesErrorShape {
  if (isRecord(err) && typeof err.code === 'string' && typeof err.message === 'string') {
    return { code: err.code, message: err.message };
  }
  return { code: 'unknown', message: err instanceof Error ? err.message : String(err) };
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    throw toReferencesError(err);
  }
}

export const referencesApi = {
  create: (draftId: string, fragmentId: number) =>
    call<Reference>('references_create', { draftId, fragmentId }),
  list: (draftId: string) => call<Reference[]>('references_list', { draftId }),
  adopt: (id: string) => call<void>('references_adopt', { id }),
  markSourceDeleted: (sourceId: string) => call<void>('references_mark_source_deleted', { sourceId }),
};
