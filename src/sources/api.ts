import { callCommand } from '../native/command-client';

/**
 * sources 命令 seam 的前端薄投影（ARCHITECTURE §4）。
 * 类型与 crates/core/src/sources.rs 的 serde 输出一一对应。
 */

export type SourceStatus = 'idle' | 'indexing';

/** 可选的间隔校验周期，与 crates/core/src/sources.rs 的 SYNC_INTERVAL_CHOICES 一致。 */
export const SYNC_INTERVAL_CHOICES = [5, 15, 30] as const;

export interface SourceSummary {
  id: string;
  name: string;
  rootPath: string;
  paused: boolean;
  status: SourceStatus;
  fileCount: number;
  ignoreGlobs: string[];
  lastSyncedAt: string | null;
  /** 间隔校验周期（分钟）：到点由后台心跳补扫一次。 */
  syncIntervalMinutes: number;
  /** 进行中的补扫进度；空闲或已落省时为 null。 */
  progress: SourceJobProgress | null;
}

/** 进行中的补扫进度（Rust JobSnapshot 的薄投影）。 */
export interface SourceJobProgress {
  phase: 'indexing' | 'completed' | 'cancelled' | 'failed';
  processed: number;
  totalFiles: number;
  fragmentCount: number;
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

export interface IndexStatus {
  sourceId: string;
  fileCount: number;
  fragmentCount: number;
  /** true 表示该来源已索引的片段与 FTS row 已 commit，search_query 必能命中。 */
  searchable: boolean;
}

export const sourcesApi = {
  add: (root: string, name: string, ignoreGlobs?: string[]) =>
    callCommand<SourceSummary>('sources_add', { root, name, ignoreGlobs }),
  list: () => callCommand<SourceSummary[]>('sources_list'),
  pause: (id: string, paused: boolean) =>
    callCommand<SourceSummary>('sources_pause', { id, paused }),
  remove: (id: string) => callCommand<void>('sources_remove', { id }),
  sync: (id: string) => callCommand<void>('sources_sync', { id }),
  reconcile: (id: string) => callCommand<void>('sources_reconcile', { id }),
  rebuildIndex: (id: string) => callCommand<void>('sources_rebuild_index', { id }),
  setSyncInterval: (id: string, intervalMinutes: number) =>
    callCommand<SourceSummary>('sources_set_sync_interval', { id, intervalMinutes }),
  awaitIndexCommitted: (sourceId: string) =>
    callCommand<IndexStatus>('sources_await_index_committed', { sourceId }),
};
