import { useCallback, useEffect, useMemo, useState } from 'react';

import { draftsApi, type DraftFile, parseConflict } from './api';
import { toCommandError, type CommandErrorShape } from '../native/command-client';

export type DraftsViewState = 'idle' | 'loading' | 'conflict';

export interface DraftsModel {
  drafts: DraftFile[];
  activeDraft: DraftFile | null;
  content: string;
  status: DraftsViewState;
  conflict: { message: string; baseline: string; local: string; external: string } | null;
  error: CommandErrorShape | null;
  actions: {
    create: (filename: string) => Promise<void>;
    open: (filename: string) => Promise<void>;
    save: () => Promise<void>;
    delete: () => Promise<void>;
    refresh: () => Promise<void>;
    resolveConflict: (choice: 'local' | 'external' | 'saveAs') => void;
    setContent: (value: string) => void;
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
  const [error, setError] = useState<CommandErrorShape | null>(null);

  const refresh = useCallback(async () => {
    try {
      setDrafts(await draftsApi.list());
    } catch (err) {
      setError(toCommandError(err));
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
        setError(toCommandError(err));
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
        setActiveDraft(draft);
        setContent('');
        await refresh();
      } catch (err) {
        setError(toCommandError(err));
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
      const shape = toCommandError(err);
      if (shape.code === 'conflict') {
        // Rust 将 Conflict 序列化为 message 字段（camelCase JSON），此处完整解析，
        // 绝不丢弃 externalContent，否则「采用外部」将无内容可用。
        const parsed = parseConflict(shape.message);
        setConflict({
          message: parsed?.message ?? shape.message,
          baseline: parsed?.baselineContent ?? '',
          local: parsed?.localContent ?? content,
          external: parsed?.externalContent ?? '',
        });
        setStatus('conflict');
      } else {
        setError(shape);
        setStatus('idle');
      }
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
      setError(toCommandError(err));
    } finally {
      setStatus('idle');
    }
  }, [activeDraft, refresh]);

  const resolveConflict = useCallback(
    async (choice: 'local' | 'external' | 'saveAs') => {
      if (!conflict || !activeDraft) {
        setConflict(null);
        setStatus('idle');
        return;
      }
      const resolution =
        choice === 'local'
          ? 'keep_local'
          : choice === 'external'
            ? 'adopt_external'
            : 'save_as_copy';
      setStatus('loading');
      try {
        // 「保留本地 / 另存副本」以用户在保存时提交的本地内容为准（已随冲突捕获，
        // 不依赖可能滞后的实时 content）；「采用外部」则由后端以磁盘文件为准。
        const local = conflict.local || content;
        const updated = await draftsApi.resolveConflict(
          activeDraft.filename,
          resolution,
          local,
        );
        setActiveDraft(updated);
        setContent(local);
        await refresh();
      } catch (err) {
        setError(toCommandError(err));
      } finally {
        setConflict(null);
        setStatus('idle');
      }
    },
    [conflict, activeDraft, content, refresh],
  );

  const setEditorContent = useCallback((value: string) => {
    setContent(value);
  }, []);

  const actions = useMemo(
    () => ({
      create,
      open,
      save,
      delete: delete_,
      refresh,
      resolveConflict,
      setContent: setEditorContent,
      dismissError: () => setError(null),
    }),
    [create, delete_, open, refresh, save, resolveConflict, setEditorContent],
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
