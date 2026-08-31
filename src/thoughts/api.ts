import { invoke } from '@tauri-apps/api/core';

/**
 * thoughts 命令 seam 的前端薄投影（ARCHITECTURE §4）。
 * 类型与 crates/core/src/thoughts.rs 的 serde 输出一一对应。
 */

export type ThoughtsErrorShape = {
  code: string;
  message: string;
};

export interface ThoughtCard {
  id: string;
  idea: string;
  sourceId: string;
  sourceName: string;
  relPath: string;
  headingPath: string;
  draftId: string;
  draftFilename: string;
  createdAt: string;
  updatedAt: string;
}

export interface UpsertThoughtCardInput {
  id?: string;
  idea: string;
  sourceId: string;
  sourceName: string;
  relPath: string;
  headingPath: string;
  draftId: string;
  draftFilename: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

export function toThoughtsError(err: unknown): ThoughtsErrorShape {
  if (isRecord(err) && typeof err.code === 'string' && typeof err.message === 'string') {
    return { code: err.code, message: err.message };
  }
  return { code: 'unknown', message: err instanceof Error ? err.message : String(err) };
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    throw toThoughtsError(err);
  }
}

export const thoughtsApi = {
  upsert: (input: UpsertThoughtCardInput) =>
    call<ThoughtCard>('thoughts_upsert', input as unknown as Record<string, unknown>),
  list: () => call<ThoughtCard[]>('thoughts_list'),
  listByDraft: (draftId: string) =>
    call<ThoughtCard[]>('thoughts_list_by_draft', { draftId }),
  listBySource: (sourceId: string) =>
    call<ThoughtCard[]>('thoughts_list_by_source', { sourceId }),
  delete: (id: string) => call<void>('thoughts_delete', { id }),
};
