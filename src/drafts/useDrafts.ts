import { useCallback, useEffect, useMemo, useState } from 'react';

import { draftsApi, type DraftFile, type DraftsErrorShape, toDraftsError } from './api';

export type DraftsViewState = 'idle' | 'loading' | 'conflict';

export interface DraftsModel {
  drafts: DraftFile[];
  activeDraft: DraftFile | null;
  content: string;
  status: DraftsViewState;
  conflict: { message: string; baseline: string; local: string; external: string } | null;
  error: DraftsErrorShape | null;
  actions: {
    create: (filename: string) => Promise<void>;
    open: (filename: string) => Promise<void>;
    save: () => Promise<void>;
    delete: () => Promise<void>;
    resolveConflict: (choice: 'local' | 'external' | 'saveAs') => void;
    dismissError: () => void;
  };
}

export function useDrafts() {
  const [drafts, setDrafts] = useState<DraftFile[]>([]);
  const [activeDraft, setActiveDraft] = useState<DraftFile | null>(null);
  const [content, setContent] = useState('');
  const [status, setStatus] = useState<DraftsViewState>('idle');
  const [conflict, setConflict] = useState<{
    message: string;
    baseline: string;
    local: string;
    external: string;
  } | null>(null);
  const [error, setError] = useState<DraftsErrorShape | null>(null);

  const refresh = useCallback(async () => {
    try {
      setDrafts(await draftsApi.list());
    } catch (err) {
      setError(toDraftsError(err));
    }
  }, []);

  useEffect(() => {
    void draftsApi.ensureDirectory();
    void refresh();
  }, [refresh]);

  const create = useCallback(
    async (filename: string) => {
      setStatus('loading');
      try {
        const draft = await draftsApi.create(filename);
        setActiveDraft(draft);
        setContent('');
        await refresh();
      } catch (err) {
        setError(toDraftsError(err));
      } finally {
        setStatus('idle');
      }
    },
    [refresh],
  );

  const open = useCallback(
    async (filename: string) => {
      setStatus('loading');
      try {
        const draft = await draftsApi.open(filename);
        // 打开后由前端读取内容（实际项目可由 Tauri fs API 读取；此处简化）
        setActiveDraft(draft);
        setContent('');
        await refresh();
      } catch (err) {
        setError(toDraftsError(err));
      } finally {
        setStatus('idle');
      }
    },
    [refresh],
  );

  const save = useCallback(async () => {
    if (!activeDraft) {
      return;
    }
    setStatus('loading');
    try {
      const updated = await draftsApi.save(activeDraft.filename, content);
      setActiveDraft(updated);
      setStatus('idle');
      await refresh();
    } catch (err) {
      const shape = toDraftsError(err);
      if (shape.code === 'conflict') {
        // 简化：前端自行构建冲突展示
        setConflict({
          message: shape.message,
          baseline: '',
          local: content,
          external: '',
        });
        setStatus('conflict');
      } else {
        setError(shape);
      }
    } finally {
      setStatus('idle');
    }
  }, [activeDraft, content, refresh]);

  const delete_ = useCallback(async () => {
    if (!activeDraft) {
      return;
    }
    setStatus('loading');
    try {
      await draftsApi.delete(activeDraft.filename);
      setActiveDraft(null);
      setContent('');
      await refresh();
    } catch (err) {
      setError(toDraftsError(err));
    } finally {
      setStatus('idle');
    }
  }, [activeDraft, refresh]);

  const resolveConflict = useCallback(
    () => {
      // TODO: 实现三选一逻辑；当前仅关闭冲突态
      setConflict(null);
      setStatus('idle');
    },
    [],
  );

  const actions = useMemo(
    () => ({
      create,
      open,
      save,
      delete: delete_,
      resolveConflict,
      dismissError: () => setError(null),
    }),
    [create, delete_, open, save, resolveConflict],
  );

  return {
    drafts,
    activeDraft,
    content,
    status,
    conflict,
    error,
    actions,
  };
}
