import { invoke } from '@tauri-apps/api/core';

/**
 * sources 命令 seam 的前端薄投影（ARCHITECTURE §4）。
 * 类型与 crates/core/src/sources.rs 的 serde 输出一一对应。
 */

export type SourceStatus = 'idle' | 'indexing';

export interface SourceSummary {
  id: string;
  name: string;
  rootPath: string;
  paused: boolean;
  status: SourceStatus;
  fileCount: number;
  ignoreGlobs: string[];
  lastSyncedAt: string | null;
}

/** sources://progress 事件载荷（Rust SourceEvent 的 serde 标签形式）。 */
export type SourcesEvent =
  | { type: 'backfill_started'; sourceId: string; totalFiles: number }
  | {
      type: 'file_indexed';
      sourceId: string;
      relPath: string;
      processed: number;
      totalFiles: number;
      fragmentCount: number;
    }
  | { type: 'backfill_completed'; sourceId: string; indexedFiles: number; totalFiles: number }
  | { type: 'backfill_cancelled'; sourceId: string; indexedFiles: number }
  | { type: 'backfill_failed'; sourceId: string; message: string };

export interface CommandErrorShape {
  code: string;
  message: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

/** 把 invoke 的 rejection 规整为稳定结构，避免到处做类型收窄。 */
export function toCommandError(err: unknown): CommandErrorShape {
  if (isRecord(err) && typeof err.code === 'string' && typeof err.message === 'string') {
    return { code: err.code, message: err.message };
  }
  return { code: 'unknown', message: err instanceof Error ? err.message : String(err) };
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    throw toCommandError(err);
  }
}

export const sourcesApi = {
  add: (root: string, name: string, ignoreGlobs?: string[]) =>
    call<SourceSummary>('sources_add', { root, name, ignoreGlobs }),
  list: () => call<SourceSummary[]>('sources_list'),
  pause: (id: string, paused: boolean) => call<SourceSummary>('sources_pause', { id, paused }),
  remove: (id: string) => call<void>('sources_remove', { id }),
  sync: (id: string) => call<void>('sources_sync', { id }),
};
