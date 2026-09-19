import { useCallback, useEffect, useMemo, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import type { UnlistenFn } from '@tauri-apps/api/event';

import { sourcesApi, type SourceSummary, type SourcesEvent } from './api';
import { DEFAULT_IGNORE_GLOBS } from './defaults';
import { toCommandError, type CommandErrorShape } from '../native/command-client';

/** 某一来源的补扫进度（事件驱动）。 */
export interface BackfillProgress {
  processed: number;
  totalFiles: number;
}

export interface SourcesActions {
  addSource: (root: string, name: string, ignoreGlobs?: string[]) => Promise<void>;
  /** 打开系统目录选择对话框；返回所选路径（取消时为 null）。 */
  pickDirectory: () => Promise<string | null>;
  pause: (id: string, paused: boolean) => Promise<void>;
  remove: (id: string) => Promise<void>;
  sync: (id: string) => Promise<void>;
  reconcile: (id: string) => Promise<void>;
  rebuildIndex: (id: string) => Promise<void>;
  setSyncInterval: (id: string, intervalMinutes: number) => Promise<void>;
  refresh: () => Promise<void>;
  dismissError: () => void;
}

export interface SourcesModel {
  sources: SourceSummary[];
  progress: Record<string, BackfillProgress>;
  defaultIgnoreGlobs: readonly string[];
  error: CommandErrorShape | null;
  actions: SourcesActions;
}

/**
 * 来源管理视图模型：命令调用 + sources://progress 事件驱动的进度状态。
 * 完成/取消/失败事件会触发列表刷新（事件是推送，列表是薄投影）。
 */
export function useSources(): SourcesModel {
  const [sources, setSources] = useState<SourceSummary[]>([]);
  const [progress, setProgress] = useState<Record<string, BackfillProgress>>({});
  const [error, setError] = useState<CommandErrorShape | null>(null);

  const refresh = useCallback(async () => {
    try {
      setSources(await sourcesApi.list());
    } catch (err) {
      setError(toCommandError(err));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let disposed = false;
    void listen<SourcesEvent>('sources://progress', (event) => {
      const payload = event.payload;
      switch (payload.type) {
        case 'backfill_started':
          setProgress((current) => ({
            ...current,
            [payload.sourceId]: { processed: 0, totalFiles: payload.totalFiles },
          }));
          break;
        case 'file_indexed':
          setProgress((current) => ({
            ...current,
            [payload.sourceId]: {
              processed: payload.processed,
              totalFiles: payload.totalFiles,
            },
          }));
          break;
        case 'backfill_completed':
        case 'backfill_cancelled':
        case 'backfill_failed':
          setProgress((current) => {
            const next = { ...current };
            delete next[payload.sourceId];
            return next;
          });
          void refresh();
          break;
      }
    }).then((fn) => {
      if (disposed) {
        fn();
      } else {
        unlisten = fn;
      }
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [refresh]);

  const actions = useMemo<SourcesActions>(() => {
    async function run(action: () => Promise<unknown>): Promise<void> {
      try {
        await action();
        await refresh();
      } catch (err) {
        setError(toCommandError(err));
      }
    }

    return {
      addSource: (root, name, ignoreGlobs) => run(() => sourcesApi.add(root, name, ignoreGlobs)),
      pickDirectory: async () => {
        try {
          const { open } = await import('@tauri-apps/plugin-dialog');
          const selected = await open({ directory: true, multiple: false });
          return typeof selected === 'string' && selected.length > 0 ? selected : null;
        } catch (err) {
          setError(toCommandError(err));
          return null;
        }
      },
      pause: (id, paused) => run(() => sourcesApi.pause(id, paused)),
      remove: (id) => run(() => sourcesApi.remove(id)),
      sync: (id) => run(() => sourcesApi.sync(id)),
      reconcile: (id) => run(() => sourcesApi.reconcile(id)),
      rebuildIndex: (id) => run(() => sourcesApi.rebuildIndex(id)),
      setSyncInterval: (id, intervalMinutes) =>
        run(() => sourcesApi.setSyncInterval(id, intervalMinutes)),
      refresh,
      dismissError: () => setError(null),
    };
  }, [refresh]);

  return { sources, progress, defaultIgnoreGlobs: DEFAULT_IGNORE_GLOBS, error, actions };
}
